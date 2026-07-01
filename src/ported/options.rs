// Copyright (c) 2008 Nicholas Marriott <nicholas.marriott@gmail.com>
//
// Permission to use, copy, modify, and distribute this software for any
// purpose with or without fee is hereby granted, provided that the above
// copyright notice and this permission notice appear in all copies.
//
// THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
// WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
// MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR
// ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
// WHATSOEVER RESULTING FROM LOSS OF MIND, USE, DATA OR PROFITS, WHETHER
// IN AN ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING
// OUT OF OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.
use crate::libc::{fnmatch};
use crate::options_table::OPTIONS_OTHER_NAMES_STR;
use crate::*;

// Option handling; each option has a name, type and value and is stored in a red-black tree.

#[repr(C)]
#[derive(Copy, Clone)]
pub struct options_array_item {
    pub index: u32,
    pub value: options_value,
    pub entry: rb_entry<options_array_item>,
}

/// C `vendor/tmux/options.c:40`: `static int options_array_cmp(struct options_array_item *a1, struct options_array_item *a2)`
fn options_array_cmp(a1: &options_array_item, a2: &options_array_item) -> cmp::Ordering {
    a1.index.cmp(&a2.index)
}
RB_GENERATE!(
    options_array,
    options_array_item,
    entry,
    discr_entry,
    options_array_cmp
);

#[repr(C)]
pub struct options_entry {
    owner: *mut options,
    name: Cow<'static, str>,
    tableentry: *const options_table_entry,
    value: options_value,
    cached: i32,
    style: style,
    entry: rb_entry<options_entry>,
}

#[repr(C)]
pub struct options {
    tree: rb_head<options_entry>,
    parent: *mut options,
}

#[expect(non_snake_case)]
#[inline]
unsafe fn OPTIONS_IS_STRING(o: *const options_entry) -> bool {
    unsafe {
        (*o).tableentry.is_null()
            || (*(*o).tableentry).type_ == options_table_type::OPTIONS_TABLE_STRING
    }
}

#[expect(non_snake_case)]
#[inline]
fn OPTIONS_IS_NUMBER(o: *const options_entry) -> bool {
    unsafe {
        !(*o).tableentry.is_null()
            && ((*(*o).tableentry).type_ == options_table_type::OPTIONS_TABLE_NUMBER
                || (*(*o).tableentry).type_ == options_table_type::OPTIONS_TABLE_KEY
                || (*(*o).tableentry).type_ == options_table_type::OPTIONS_TABLE_COLOUR
                || (*(*o).tableentry).type_ == options_table_type::OPTIONS_TABLE_FLAG
                || (*(*o).tableentry).type_ == options_table_type::OPTIONS_TABLE_CHOICE)
    }
}

#[expect(non_snake_case)]
#[inline]
unsafe fn OPTIONS_IS_COMMAND(o: *const options_entry) -> bool {
    unsafe {
        !(*o).tableentry.is_null()
            && (*(*o).tableentry).type_ == options_table_type::OPTIONS_TABLE_COMMAND
    }
}

#[expect(non_snake_case)]
#[inline]
unsafe fn OPTIONS_IS_ARRAY(o: *const options_entry) -> bool {
    unsafe {
        !(*o).tableentry.is_null() && ((*(*o).tableentry).flags & OPTIONS_TABLE_IS_ARRAY) != 0
    }
}

RB_GENERATE!(options_tree, options_entry, entry, discr_entry, options_cmp);

/// C `vendor/tmux/options.c:93`: `static int options_cmp(struct options_entry *lhs, struct options_entry *rhs)`
fn options_cmp(lhs: &options_entry, rhs: &options_entry) -> cmp::Ordering {
    lhs.name.cmp(&rhs.name)
}

/// C `vendor/tmux/options.c:99`: `static const char *options_map_name(const char *name)`
fn options_map_name(name: &str) -> Option<&'static str> {
    for &options_name_map { from, to} in &OPTIONS_OTHER_NAMES {
        if from == name {
            return Some(to);
        }
    }
    None
}

fn options_map_name_str(name: &str) -> &str {
    for map in &OPTIONS_OTHER_NAMES_STR {
        if map.from == name {
            return map.to;
        }
    }
    name
}

/// C `vendor/tmux/options.c:111`: `static const struct options_table_entry *options_parent_table_entry(struct options *oo, const char *s)`
unsafe fn options_parent_table_entry(
    oo: *mut options,
    s: &str,
) -> *const options_table_entry {
    unsafe {
        if (*oo).parent.is_null() {
            fatalx_!("no parent options for {s}");
        }

        let o = options_get(&mut *(*oo).parent, s);
        if o.is_null() {
            fatalx_!("{s} not in parent options");
        }

        (*o).tableentry
    }
}

/// C `vendor/tmux/options.c:124`: `static void options_value_free(struct options_entry *o, union options_value *ov)`
unsafe fn options_value_free(o: *const options_entry, ov: *mut options_value) {
    unsafe {
        if OPTIONS_IS_STRING(o) {
            free_((*ov).string);
        }
        if OPTIONS_IS_COMMAND(o) && !(*ov).cmdlist.is_null() {
            cmd_list_free((*ov).cmdlist);
        }
    }
}

/// C `vendor/tmux/options.c:133`: `static char *options_value_to_string(struct options_entry *o, union options_value *ov, int numeric)`
unsafe fn options_value_to_string(
    o: *mut options_entry,
    ov: *mut options_value,
    numeric: i32,
) -> *mut u8 {
    unsafe {
        if OPTIONS_IS_COMMAND(o) {
            return cmd_list_print(&*(*ov).cmdlist, 0);
        }

        if OPTIONS_IS_NUMBER(o) {
            let s = match (*(*o).tableentry).type_ {
                options_table_type::OPTIONS_TABLE_NUMBER => {
                    format_nul!("{}", (*ov).number)
                }
                options_table_type::OPTIONS_TABLE_KEY => {
                    xstrdup(key_string_lookup_key((*ov).number as u64, 0)).as_ptr()
                }
                options_table_type::OPTIONS_TABLE_COLOUR => {
                    CString::new(colour_tostring((*ov).number as i32).into_owned())
                        .unwrap()
                        .into_raw()
                        .cast()
                }
                options_table_type::OPTIONS_TABLE_FLAG => {
                    if numeric != 0 {
                        format_nul!("{}", (*ov).number)
                    } else {
                        xstrdup(if (*ov).number != 0 {
                            c!("on")
                        } else {
                            c!("off")
                        })
                        .as_ptr()
                    }
                }
                options_table_type::OPTIONS_TABLE_CHOICE => {
                    xstrdup__((*(*o).tableentry).choices[(*ov).number as usize])
                }
                _ => {
                    fatalx("not a number option type");
                }
            };
            return s;
        }

        if OPTIONS_IS_STRING(o) {
            return xstrdup((*ov).string).as_ptr();
        }

        xstrdup(c!("")).as_ptr()
    }
}

/// C `vendor/tmux/options.c:171`: `struct options *options_create(struct options *parent)`
pub unsafe fn options_create(parent: *mut options) -> *mut options {
    unsafe {
        let oo = xcalloc1::<options>() as *mut options;
        rb_init(&raw mut (*oo).tree);
        (*oo).parent = parent;
        oo
    }
}

/// C `vendor/tmux/options.c:182`: `void options_free(struct options *oo)`
pub unsafe fn options_free(oo: *mut options) {
    unsafe {
        for o in rb_foreach(&raw mut (*oo).tree) {
            options_remove(o.as_ptr());
        }
        free_(oo);
    }
}

/// C `vendor/tmux/options.c:192`: `struct options *options_get_parent(struct options *oo)`
pub unsafe fn options_get_parent(oo: *mut options) -> *mut options {
    unsafe { (*oo).parent }
}

/// C `vendor/tmux/options.c:198`: `void options_set_parent(struct options *oo, struct options *parent)`
pub fn options_set_parent(oo: &mut options, parent: *mut options) {
    oo.parent = parent;
}

/// C `vendor/tmux/options.c:204`: `struct options_entry *options_first(struct options *oo)`
pub unsafe fn options_first(oo: *mut options) -> *mut options_entry {
    unsafe { rb_min(&raw mut (*oo).tree) }
}

/// C `vendor/tmux/options.c:210`: `struct options_entry *options_next(struct options_entry *o)`
pub unsafe fn options_next(o: *mut options_entry) -> *mut options_entry {
    unsafe { rb_next(o) }
}

/// C `vendor/tmux/options.c:216`: `struct options_entry *options_get_only(struct options *oo, const char *name)`
pub unsafe fn options_get_only(oo: *mut options, name: &str) -> *mut options_entry {
    unsafe {
        let name = std::mem::transmute::<&str, &'static str>(name);
        let mut o = options_entry {
            name: Cow::Borrowed(name),
            ..zeroed() // TODO use uninit
        };

        let found = rb_find(&raw mut (*oo).tree, &raw const o);
        if found.is_null() {
            o.name = Cow::Borrowed(options_map_name(name).unwrap_or(name));
            rb_find(&raw mut (*oo).tree, &o)
        } else {
            found
        }
    }
}
// consider to remove this one or the other
#[expect(dead_code)]
unsafe fn options_get_only_(oo: *mut options, name: &str) -> *mut options_entry {
    unsafe {
        let found = rb_find_by(&raw mut (*oo).tree, |oe| {
            (*oe.name).cmp(name).reverse()
        });
        if found.is_null() {
            let name = options_map_name_str(name);
            rb_find_by(&raw mut (*oo).tree, |oe| {
                (*oe.name).cmp(name).reverse()
            })
        } else {
            found
        }
    }
}

pub unsafe fn options_get_only_const(oo: *const options, name: &str) -> *const options_entry {
    unsafe {
        let found = rb_find_by_const(&(*oo).tree, |oe| (*oe.name).cmp(name).reverse());
        if found.is_null() {
            let name = options_map_name_str(name);
            rb_find_by_const(&(*oo).tree, |oe| (*oe.name).cmp(name).reverse())
        } else {
            found
        }
    }
}

/// C `vendor/tmux/options.c:229`: `struct options_entry *options_get(struct options *oo, const char *name)`
pub fn options_get(oo: &mut options, name: &str) -> *mut options_entry {
    #[expect(clippy::shadow_same)]
    let mut oo: *mut options = oo;

    unsafe {
        let mut o = options_get_only(oo, name);
        while o.is_null() {
            oo = (*oo).parent;
            if oo.is_null() {
                break;
            }
            o = options_get_only(oo, name);
        }
        o
    }
}

unsafe fn options_get_const(mut oo: *const options, name: &str) -> *const options_entry {
    unsafe {
        let mut o;
        while {
            o = options_get_only_const(oo, name);
            o.is_null()
        } {
            oo = (*oo).parent;
            if oo.is_null() {
                break;
            }
        }
        o
    }
}

/// C `vendor/tmux/options.c:244`: `struct options_entry *options_empty(struct options *oo, const struct options_table_entry *oe)`
pub unsafe fn options_empty(
    oo: *mut options,
    oe: *const options_table_entry,
) -> *mut options_entry {
    unsafe {
        let o = options_add(oo, (*oe).name);
        (*o).tableentry = oe;

        if (*oe).flags & OPTIONS_TABLE_IS_ARRAY != 0 {
            rb_init(&raw mut (*o).value.array);
        }
        o
    }
}

/// C `vendor/tmux/options.c:258`: `struct options_entry *options_default(struct options *oo, const struct options_table_entry *oe)`
pub unsafe fn options_default(
    oo: *mut options,
    oe: *const options_table_entry,
) -> *mut options_entry {
    unsafe {
        let o = options_empty(oo, oe);
        let ov = &raw mut (*o).value;

        if (*oe).flags & OPTIONS_TABLE_IS_ARRAY != 0 {
            if (*oe).default_arr.is_null() {
                _ = options_array_assign(o, (*oe).default_str.unwrap());
                return o;
            }
            let mut i = 0usize;
            while !(*(*oe).default_arr.add(i)).is_null() {
                _ = options_array_set(
                    o,
                    i as u32,
                    Some(cstr_to_str(*(*oe).default_arr.add(i))),
                    false,
                );
                i += 1;
            }
            return o;
        }

        match (*oe).type_ {
            options_table_type::OPTIONS_TABLE_STRING => {
                (*ov).string = xstrdup___((*oe).default_str);
            }
            _ => {
                (*ov).number = (*oe).default_num;
            }
        }
        o
    }
}

/// C `vendor/tmux/options.c:301`: `char *options_default_to_string(const struct options_table_entry *oe)`
pub unsafe fn options_default_to_string(oe: *const options_table_entry) -> NonNull<u8> {
    unsafe {
        match (*oe).type_ {
            options_table_type::OPTIONS_TABLE_STRING
            | options_table_type::OPTIONS_TABLE_COMMAND => {
                NonNull::new_unchecked(xstrdup___((*oe).default_str))
            }
            options_table_type::OPTIONS_TABLE_NUMBER => {
                NonNull::new(format_nul!("{}", (*oe).default_num)).unwrap()
            }
            options_table_type::OPTIONS_TABLE_KEY => {
                xstrdup(key_string_lookup_key((*oe).default_num as u64, 0))
            }
            options_table_type::OPTIONS_TABLE_COLOUR => NonNull::new(
                CString::new(colour_tostring((*oe).default_num as i32).into_owned())
                    .unwrap()
                    .into_raw()
                    .cast(),
            )
            .unwrap(),
            options_table_type::OPTIONS_TABLE_FLAG => xstrdup_(if (*oe).default_num != 0 {
                c"on"
            } else {
                c"off"
            }),
            options_table_type::OPTIONS_TABLE_CHOICE => {
                NonNull::new(xstrdup__((*oe).choices[(*oe).default_num as usize])).unwrap()
            }
        }
    }
}

/// C `vendor/tmux/options.c:332`: `static struct options_entry *options_add(struct options *oo, const char *name)`
unsafe fn options_add(oo: *mut options, name: &str) -> *mut options_entry {
    unsafe {
        let mut o = options_get_only(oo, name);
        if !o.is_null() {
            options_remove(o);
        }

        o = Box::into_raw(Box::new(
            options_entry {
                owner: oo,
                name: Cow::Owned(name.to_string()),
                tableentry: null(),
                value: options_value {number: 0},
                cached: 0,
                style: zeroed(),
                entry: rb_entry::default(),
            }
        ));

        rb_insert(&raw mut (*oo).tree, o);
        o
    }
}

/// C `vendor/tmux/options.c:349`: `static void options_remove(struct options_entry *o)`
unsafe fn options_remove(o: *mut options_entry) {
    unsafe {
        let oo = (*o).owner;

        if options_is_array(o) {
            options_array_clear(o);
        } else {
            options_value_free(o, &mut (*o).value);
        }
        rb_remove(&mut (*oo).tree, o);
        (*o).name = Cow::Borrowed("");
        free_(o);
    }
}

/// C `vendor/tmux/options.c:363`: `const char *options_name(struct options_entry *o)`
pub unsafe fn options_name<'a>(o: *mut options_entry) -> &'a str {
    unsafe { &(*o).name }
}

/// C `vendor/tmux/options.c:369`: `struct options *options_owner(struct options_entry *o)`
pub unsafe fn options_owner(o: *mut options_entry) -> *mut options {
    unsafe { (*o).owner }
}

/// C `vendor/tmux/options.c:375`: `const struct options_table_entry *options_table_entry(struct options_entry *o)`
pub unsafe fn options_table_entry(o: *mut options_entry) -> *const options_table_entry {
    unsafe { (*o).tableentry }
}

/// C `vendor/tmux/options.c:381`: `static struct options_array_item *options_array_item(struct options_entry *o, u_int idx)`
unsafe fn options_array_item(o: *mut options_entry, idx: c_uint) -> *mut options_array_item {
    unsafe {
        let mut a = options_array_item {
            index: idx,
            ..zeroed() // TODO use uninit
        };
        rb_find(&raw mut (*o).value.array, &raw mut a)
    }
}

/// C `vendor/tmux/options.c:390`: `static struct options_array_item *options_array_new(struct options_entry *o, u_int idx)`
unsafe fn options_array_new(o: *mut options_entry, idx: c_uint) -> *mut options_array_item {
    unsafe {
        let a = xcalloc1::<options_array_item>() as *mut options_array_item;
        (*a).index = idx;
        rb_insert(&mut (*o).value.array, a);
        a
    }
}

/// C `vendor/tmux/options.c:401`: `static void options_array_free(struct options_entry *o, struct options_array_item *a)`
unsafe fn options_array_free(o: *mut options_entry, a: *mut options_array_item) {
    unsafe {
        options_value_free(o, &mut (*a).value);
        rb_remove(&mut (*o).value.array, a);
        free_(a);
    }
}

/// C `vendor/tmux/options.c:409`: `void options_array_clear(struct options_entry *o)`
pub unsafe fn options_array_clear(o: *mut options_entry) {
    unsafe {
        if !options_is_array(o) {
            return;
        }

        let mut a = rb_min(&raw mut (*o).value.array);
        while !a.is_null() {
            let next: *mut options_array_item = rb_next(a);
            options_array_free(o, a);
            a = next;
        }
    }
}

/// C `vendor/tmux/options.c:421`: `union options_value *options_array_get(struct options_entry *o, u_int idx)`
pub unsafe fn options_array_get(o: *mut options_entry, idx: u32) -> *mut options_value {
    unsafe {
        if !options_is_array(o) {
            return null_mut();
        }
        let a = options_array_item(o, idx);
        if a.is_null() {
            return null_mut();
        }
        &raw mut (*a).value
    }
}

/// C `vendor/tmux/options.c:434`: `int options_array_set(struct options_entry *o, u_int idx, const char *value, int append, char **cause)`
pub unsafe fn options_array_set(
    o: *mut options_entry,
    idx: u32,
    value: Option<&str>,
    append: bool,
) -> Result<(), CString> {
    unsafe {
        if !OPTIONS_IS_ARRAY(o) {
            return Err(CString::new("not an array").unwrap());
        }

        let Some(value) = value else {
            let a = options_array_item(o, idx);
            if !a.is_null() {
                options_array_free(o, a);
            }
            return Ok(());
        };

        if OPTIONS_IS_COMMAND(o) {
            let cmdlist = match cmd_parse_from_string(value, None) {
                Err(error) => {
                    return Err(CString::from_raw(error.cast()));
                }
                Ok(cmdlist) => cmdlist,
            };

            let mut a = options_array_item(o, idx);
            if a.is_null() {
                a = options_array_new(o, idx);
            } else {
                options_value_free(o, &raw mut (*a).value);
            }
            (*a).value.cmdlist = cmdlist;
            return Ok(());
        }

        if OPTIONS_IS_STRING(o) {
            let mut a = options_array_item(o, idx);
            let new = if !a.is_null() && append {
                format_nul!("{}{}", _s((*a).value.string), value)
            } else {
                xstrdup__(value)
            };

            if a.is_null() {
                a = options_array_new(o, idx);
            } else {
                options_value_free(o, &mut (*a).value);
            }
            (*a).value.string = new;
            return Ok(());
        }

        if !(*o).tableentry.is_null()
            && (*(*o).tableentry).type_ == options_table_type::OPTIONS_TABLE_COLOUR
        {
            let number = colour_fromstring(value);
            if number == -1 {
                return Err(CString::new(format!("bad colour: {value}")).unwrap());
            }
            let mut a = options_array_item(o, idx);
            if a.is_null() {
                a = options_array_new(o, idx);
            } else {
                options_value_free(o, &raw mut (*a).value);
            }
            (*a).value.number = number as i64;
            return Ok(());
        }

        Err(CString::new("wrong array type").unwrap())
    }
}

// note one difference was that this function previously could avoid allocation on error
/// C `vendor/tmux/options.c:511`: `int options_array_assign(struct options_entry *o, const char *s, char **cause)`
pub unsafe fn options_array_assign(o: *mut options_entry, s: &str) -> Result<(), CString> {
    unsafe {
        let mut separator = (*(*o).tableentry).separator;
        if separator.is_null() {
            separator = c!(" ,");
        }
        if *separator == 0 {
            if s.is_empty() {
                return Ok(());
            }
            let mut i = 0;
            while i < u32::MAX {
                if options_array_item(o, i).is_null() {
                    break;
                }
                i += 1;
            }
            return options_array_set(o, i, Some(s), false);
        }

        if s.is_empty() {
            return Ok(());
        }
        let copy = xstrdup__(s);
        let mut string = copy;
        while let Some(next) = NonNull::new(strsep(&raw mut string, separator)) {
            let next = next.as_ptr();
            if *next == 0 {
                continue;
            }
            let mut i = 0;
            while i < u32::MAX {
                if options_array_item(o, i).is_null() {
                    break;
                }
                i += 1;
            }
            if i == u32::MAX {
                break;
            }
            if let Err(cause) = options_array_set(o, i, Some(cstr_to_str(next)), false) {
                free_(copy);
                return Err(cause);
            }
        }
        free_(copy);
        Ok(())
    }
}

/// C `vendor/tmux/options.c:552`: `struct options_array_item *options_array_first(struct options_entry *o)`
pub unsafe fn options_array_first(o: *mut options_entry) -> *mut options_array_item {
    unsafe {
        if !OPTIONS_IS_ARRAY(o) {
            return null_mut();
        }
        rb_min(&raw mut (*o).value.array)
    }
}

/// C `vendor/tmux/options.c:560`: `struct options_array_item *options_array_next(struct options_array_item *a)`
pub unsafe fn options_array_next(a: *mut options_array_item) -> *mut options_array_item {
    unsafe { rb_next(a) }
}

/// C `vendor/tmux/options.c:566`: `u_int options_array_item_index(struct options_array_item *a)`
pub unsafe fn options_array_item_index(a: *mut options_array_item) -> u32 {
    unsafe { (*a).index }
}

/// C `vendor/tmux/options.c:572`: `union options_value *options_array_item_value(struct options_array_item *a)`
pub unsafe fn options_array_item_value(a: *mut options_array_item) -> *mut options_value {
    unsafe { &raw mut (*a).value }
}

/// C `vendor/tmux/options.c:578`: `int options_is_array(struct options_entry *o)`
pub unsafe fn options_is_array(o: *mut options_entry) -> bool {
    unsafe { OPTIONS_IS_ARRAY(o) }
}

/// C `vendor/tmux/options.c:584`: `int options_is_string(struct options_entry *o)`
pub unsafe fn options_is_string(o: *mut options_entry) -> bool {
    unsafe { OPTIONS_IS_STRING(o) }
}

/// C `vendor/tmux/options.c:590`: `char *options_to_string(struct options_entry *o, int idx, int numeric)`
pub unsafe fn options_to_string(o: *mut options_entry, idx: i32, numeric: i32) -> *mut u8 {
    unsafe {
        if OPTIONS_IS_ARRAY(o) {
            if idx == -1 {
                let mut result = null_mut();
                let mut last: *mut u8 = null_mut();

                let mut a = rb_min(&raw mut (*o).value.array);
                while !a.is_null() {
                    let next = options_value_to_string(
                        o,
                        &raw mut (*a.cast::<options_array_item>()).value,
                        numeric,
                    );

                    if last.is_null() {
                        result = next;
                    } else {
                        let new_result = format_nul!("{} {}", _s(last), _s(next));
                        free_(last);
                        free_(next);
                        result = new_result;
                    }
                    last = result;

                    a = rb_next(a);
                }

                if result.is_null() {
                    return xstrdup(c!("")).as_ptr();
                }
                return result;
            }

            let a = options_array_item(o, idx as u32);
            if a.is_null() {
                return xstrdup(c!("")).as_ptr();
            }
            return options_value_to_string(o, &raw mut (*a).value, numeric);
        }

        options_value_to_string(o, &raw mut (*o).value, numeric)
    }
}

/// C `vendor/tmux/options.c:624`: `char *options_parse(const char *name, int *idx)`
pub fn options_parse(name: &str) -> Option<(String, i32)> {
    if name.is_empty() {
        return None;
    }

    let mut copy = name.to_string();

    let Some(cp) = copy.find('[') else {
        return Some((copy, -1));
    };

    let end = copy[cp+1..].find(']').map(|end| end + cp + 1)?;

    if end != copy.len() - 1 || !copy.as_bytes()[end - 1].is_ascii_digit() {
        return None;
    }

    let Ok(parsed_idx) = copy[cp+1..end].parse::<i32>() else {
        return None;
    };

    copy.truncate(cp);
    Some((copy, parsed_idx))
}

/// C `vendor/tmux/options.c:649`: `struct options_entry *options_parse_get(struct options *oo, const char *s, int *idx, int only)`
pub unsafe fn options_parse_get(
    oo: *mut options,
    s: &str,
    idx: *mut i32,
    only: i32,
) -> *mut options_entry {
    unsafe {
        let Some((name, idx_value)) = options_parse(s) else {
            return null_mut();
        };
        *idx = idx_value;

        if only != 0 {
            options_get_only(oo, &name)
        } else {
            options_get(&mut *oo, &name)
        }
    }
}

/// C `vendor/tmux/options.c:678`: `char *options_match(const char *s, int *idx, int *ambiguous)`
pub unsafe fn options_match(s: &str, idx: *mut i32, ambiguous: *mut i32) -> Option<String> {
    unsafe {
        let (parsed, idx_value) = options_parse(s)?;
        *idx = idx_value;

        if parsed.starts_with('@') {
            *ambiguous = 0;
            return Some(parsed);
        }

        let name = options_map_name(&parsed).unwrap_or(&parsed);

        let mut found: *const options_table_entry = null();

        for oe in &OPTIONS_TABLE {
            if oe.name == name {
                found = oe;
                break;
            }
            if oe.name.starts_with(name) {
                if !found.is_null() {
                    *ambiguous = 1;
                    return None;
                }
                found = oe;
            }
        }

        if found.is_null() {
            *ambiguous = 0;
            return None;
        }

        Some((*found).name.to_string())
    }
}

#[expect(dead_code)]
/// C `vendor/tmux/options.c:720`: `struct options_entry *options_match_get(struct options *oo, const char *s, int *idx, int only, int *ambiguous)`
unsafe fn options_match_get(
    oo: *mut options,
    s: &str,
    idx: *mut i32,
    only: i32,
    ambiguous: *mut i32,
) -> *mut options_entry {
    unsafe {
        let Some(name) = options_match(s, idx, ambiguous) else {
            return null_mut();
        };

        *ambiguous = 0;
        if only != 0 {
            options_get_only(oo, &name)
        } else {
            options_get(&mut *oo, &name)
        }
    }
}

/// C `vendor/tmux/options.c:739`: `const char *options_get_string(struct options *oo, const char *name)`
pub unsafe fn options_get_string(oo: *mut options, name: &str) -> *const u8 {
    unsafe {
        let o = options_get(&mut *oo, name);
        if o.is_null() {
            fatalx_!("missing option {name}");
        }
        if !OPTIONS_IS_STRING(o) {
            fatalx_!("option {name} is not a string");
        }
        (*o).value.string
    }
}

pub unsafe fn options_get_string_(oo: *const options, name: &str) -> *const u8 {
    unsafe {
        let o = options_get_const(oo, name);
        if o.is_null() {
            fatalx_!("missing option {name}");
        }
        if !OPTIONS_IS_STRING(o) {
            fatalx_!("option {name} is not a string");
        }
        (*o).value.string
    }
}

/// C `vendor/tmux/options.c:752`: `long long options_get_number(struct options *oo, const char *name)`
unsafe fn options_get_number(oo: *mut options, name: &str) -> i64 {
    unsafe {
        let o = options_get(&mut *oo, name);
        if o.is_null() {
            fatalx_!("missing option {name}");
        }
        if !OPTIONS_IS_NUMBER(o) {
            fatalx_!("option {name} is not a number");
        }
        (*o).value.number
    }
}

pub unsafe fn options_get_number_(oo: *const options, name: &str) -> i64 {
    unsafe {
        let o = options_get_const(oo, name);
        if o.is_null() {
            fatalx_!("missing option {name}");
        }
        if !OPTIONS_IS_NUMBER(o) {
            fatalx_!("option {name} is not a number");
        }
        (*o).value.number
    }
}

/// panics if internally stored value is out of range of returned type
#[track_caller]
pub fn options_get_number___<T: TryFrom<i64>>(oo: &options, name: &str) -> T {
    unsafe {
        let o = options_get_const(oo, name);
        if o.is_null() {
            panic!("missing option {name}");
        }
        if !OPTIONS_IS_NUMBER(o) {
            panic!("option {name} is not a number");
        }

        match T::try_from((*o).value.number) {
            Ok(value) => value,
            Err(_) => panic!("options_get_number out of range"),
        }
    }
}

macro_rules! options_set_string {
   ($oo:expr, $name:expr, $append:expr, $fmt:literal $(, $args:expr)* $(,)?) => {
        crate::options_::options_set_string_($oo, $name, $append, format_args!($fmt $(, $args)*))
    };
}
pub(crate) use options_set_string;

pub unsafe fn options_set_string_(
    oo: *mut options,
    name: &str,
    append: bool,
    args: std::fmt::Arguments,
) -> *mut options_entry {
    unsafe {
        let mut separator = c!("");
        let value: *mut u8;

        let mut s = args.to_string();
        s.push('\0');
        let s = s.leak().as_mut_ptr().cast();

        let mut o = options_get_only(oo, name);
        if !o.is_null() && append && OPTIONS_IS_STRING(o) {
            if !name.starts_with('@') {
                separator = (*(*o).tableentry).separator;
                if separator.is_null() {
                    separator = c!("");
                }
            }
            value = format_nul!("{}{}{}", _s((*o).value.string), _s(separator), _s(s),);
            free_(s);
        } else {
            value = s;
        }

        if o.is_null() && name.starts_with('@') {
            o = options_add(oo, name);
        } else if o.is_null() {
            o = options_default(oo, options_parent_table_entry(oo, name));
            if o.is_null() {
                return null_mut();
            }
        }

        if !OPTIONS_IS_STRING(o) {
            panic!("option {name} is not a string");
        }
        free_((*o).value.string);
        (*o).value.string = value;
        (*o).cached = 0;
        o
    }
}

/// C `vendor/tmux/options.c:818`: `struct options_entry *options_set_number(struct options *oo, const char *name, long long value)`
pub unsafe fn options_set_number(
    oo: *mut options,
    name: &str,
    value: i64,
) -> *mut options_entry {
    unsafe {
        if name.starts_with('@') {
            panic!("user option {name} must be a string");
        }

        let mut o = options_get_only(oo, name);
        if o.is_null() {
            o = options_default(oo, options_parent_table_entry(oo, name));
            if o.is_null() {
                return null_mut();
            }
        }

        if !OPTIONS_IS_NUMBER(o) {
            panic!("option {name} is not a number");
        }
        (*o).value.number = value;
        o
    }
}

/// C `vendor/tmux/options.c:863`: `int options_scope_from_name(struct args *args, int window, const char *name, struct cmd_find_state *fs, struct options **oo, char **cause)`
pub unsafe fn options_scope_from_name(
    args: *mut args,
    window: i32,
    name: &str,
    fs: *mut cmd_find_state,
    oo: *mut *mut options,
    cause: *mut *mut u8,
) -> i32 {
    unsafe {
        let s = (*fs).s;
        let wl = (*fs).wl;
        let wp = (*fs).wp;
        let target = args_get_(args, 't');
        let mut scope = OPTIONS_TABLE_NONE;

        if name.starts_with('@') {
            return options_scope_from_flags(args, window, fs, oo, cause);
        }

        let Some(oe) = OPTIONS_TABLE.iter().find(|oe| oe.name == name) else {
            *cause = format_nul!("unknown option: {name}");
            return OPTIONS_TABLE_NONE;
        };

        const OPTIONS_TABLE_WINDOW_AND_PANE: i32 = OPTIONS_TABLE_WINDOW | OPTIONS_TABLE_PANE;
        match oe.scope {
            OPTIONS_TABLE_SERVER => {
                *oo = GLOBAL_OPTIONS;
                scope = OPTIONS_TABLE_SERVER;
            }
            OPTIONS_TABLE_SESSION => {
                if args_has(args, 'g') {
                    *oo = GLOBAL_S_OPTIONS;
                    scope = OPTIONS_TABLE_SESSION;
                } else if s.is_null() && !target.is_null() {
                    *cause = format_nul!("no such session: {}", _s(target));
                } else if s.is_null() {
                    *cause = format_nul!("no current session");
                } else {
                    *oo = (*s).options;
                    scope = OPTIONS_TABLE_SESSION;
                }
            }
            OPTIONS_TABLE_WINDOW_AND_PANE => {
                if args_has(args, 'p') {
                    if wp.is_null() && !target.is_null() {
                        *cause = format_nul!("no such pane: {}", _s(target));
                    } else if wp.is_null() {
                        *cause = format_nul!("no current pane");
                    } else {
                        *oo = (*wp).options;
                        scope = OPTIONS_TABLE_PANE;
                    }
                } else {
                    // FALLTHROUGH same as OPTIONS_TABLE_WINDOW case
                    if args_has(args, 'g') {
                        *oo = GLOBAL_W_OPTIONS;
                        scope = OPTIONS_TABLE_WINDOW;
                    } else if wl.is_null() && !target.is_null() {
                        *cause = format_nul!("no such window: {}", _s(target));
                    } else if wl.is_null() {
                        *cause = format_nul!("no current window");
                    } else {
                        *oo = (*(*wl).window).options;
                        scope = OPTIONS_TABLE_WINDOW;
                    }
                }
            }
            OPTIONS_TABLE_WINDOW => {
                if args_has(args, 'g') {
                    *oo = GLOBAL_W_OPTIONS;
                    scope = OPTIONS_TABLE_WINDOW;
                } else if wl.is_null() && !target.is_null() {
                    *cause = format_nul!("no such window: {}", _s(target));
                } else if wl.is_null() {
                    *cause = format_nul!("no current window");
                } else {
                    *oo = (*(*wl).window).options;
                    scope = OPTIONS_TABLE_WINDOW;
                }
            }
            _ => {}
        }
        scope
    }
}

/// C `vendor/tmux/options.c:934`: `int options_scope_from_flags(struct args *args, int window, struct cmd_find_state *fs, struct options **oo, char **cause)`
pub unsafe fn options_scope_from_flags(
    args: *mut args,
    window: i32,
    fs: *mut cmd_find_state,
    oo: *mut *mut options,
    cause: *mut *mut u8,
) -> i32 {
    unsafe {
        let s = (*fs).s;
        let wl = (*fs).wl;
        let wp = (*fs).wp;
        let target = args_get_(args, 't');

        if args_has(args, 's') {
            *oo = GLOBAL_OPTIONS;
            return OPTIONS_TABLE_SERVER;
        }

        if args_has(args, 'p') {
            if wp.is_null() {
                if !target.is_null() {
                    *cause = format_nul!("no such pane: {}", _s(target));
                } else {
                    *cause = format_nul!("no current pane");
                }
                return OPTIONS_TABLE_NONE;
            }
            *oo = (*wp).options;
            OPTIONS_TABLE_PANE
        } else if window != 0 || args_has(args, 'w') {
            if args_has(args, 'g') {
                *oo = GLOBAL_W_OPTIONS;
                return OPTIONS_TABLE_WINDOW;
            }
            if wl.is_null() {
                if !target.is_null() {
                    *cause = format_nul!("no such window: {}", _s(target));
                } else {
                    *cause = format_nul!("no current window");
                }
                return OPTIONS_TABLE_NONE;
            }
            *oo = (*(*wl).window).options;
            OPTIONS_TABLE_WINDOW
        } else {
            if args_has(args, 'g') {
                *oo = GLOBAL_S_OPTIONS;
                return OPTIONS_TABLE_SESSION;
            }
            if s.is_null() {
                if !target.is_null() {
                    *cause = format_nul!("no such session: {}", _s(target));
                } else {
                    *cause = format_nul!("no current session");
                }
                return OPTIONS_TABLE_NONE;
            }
            *oo = (*s).options;
            OPTIONS_TABLE_SESSION
        }
    }
}

/// C `vendor/tmux/options.c:989`: `struct style *options_string_to_style(struct options *oo, const char *name, struct format_tree *ft)`
pub unsafe fn options_string_to_style(
    oo: *mut options,
    name: &str,
    ft: *mut format_tree,
) -> *mut style {
    let __func__ = c!("options_string_to_style");
    unsafe {
        let o = options_get(&mut *oo, name);
        if o.is_null() || !OPTIONS_IS_STRING(o) {
            return null_mut();
        }

        if (*o).cached != 0 {
            return &mut (*o).style;
        }
        let s = (*o).value.string;
        log_debug!("{}: {} is '{}'", _s(__func__), name, _s(s));

        style_set(&mut (*o).style, &GRID_DEFAULT_CELL);
        (*o).cached = cstr_to_str(s).contains("#{") as i32;

        if !ft.is_null() && (*o).cached == 0 {
            let expanded = format_expand(ft, s);
            if style_parse(&mut (*o).style, &GRID_DEFAULT_CELL, expanded) != 0 {
                free_(expanded);
                return null_mut();
            }
            free_(expanded);
        } else if style_parse(&mut (*o).style, &GRID_DEFAULT_CELL, s) != 0 {
            return null_mut();
        }
        &mut (*o).style
    }
}

/// C `vendor/tmux/options.c:1033`: `static int options_from_string_check(const struct options_table_entry *oe, const char *value, char **cause)`
unsafe fn options_from_string_check(
    oe: *const options_table_entry,
    value: *const u8,
) -> Result<(), CString> {
    unsafe {
        let mut sy: style = std::mem::zeroed();

        if oe.is_null() {
            return Ok(());
        }
        if (*oe).name == "default-shell" && !checkshell_(value) {
            return Err(CString::new(format!("not a suitable shell: {}", _s(value))).unwrap());
        }
        if !(*oe).pattern.is_null() && fnmatch((*oe).pattern, value, 0) != 0 {
            return Err(CString::new(format!("value is invalid: {}", _s(value))).unwrap());
        }
        if ((*oe).flags & OPTIONS_TABLE_IS_STYLE) != 0
            && !cstr_to_str(value).contains("#{")
            && style_parse(&mut sy, &GRID_DEFAULT_CELL, value) != 0
        {
            return Err(CString::new(format!("invalid style: {}", _s(value))).unwrap());
        }
        Ok(())
    }
}

/// C `vendor/tmux/options.c:1064`: `static int options_from_string_flag(struct options *oo, const char *name, const char *value, char **cause)`
unsafe fn options_from_string_flag(
    oo: *mut options,
    name: &str,
    value: *const u8,
) -> Result<(), CString> {
    unsafe {
        let flag = if value.is_null() || *value == 0 {
            options_get_number(oo, name) == 0
        } else if streq_(value, "1") || strcaseeq_(value, "on") || strcaseeq_(value, "yes") {
            true
        } else if streq_(value, "0") || strcaseeq_(value, "off") || strcaseeq_(value, "no") {
            false
        } else {
            return Err(CString::new(format!("bad value: {}", _s(value))).unwrap());
        };
        options_set_number(oo, name, flag as i64);
        Ok(())
    }
}

/// C `vendor/tmux/options.c:1088`: `int options_find_choice(const struct options_table_entry *oe, const char *value, char **cause)`
pub unsafe fn options_find_choice(
    oe: *const options_table_entry,
    value: *const u8,
) -> Result<i32, CString> {
    unsafe {
        let Some(choice) = (*oe).choices.iter().position(|&cp| streq_(value, cp)) else {
            return Err(CString::new(format!("unknown value: {}", _s(value))).unwrap());
        };
        Ok(choice as i32)
    }
}

/// C `vendor/tmux/options.c:1107`: `static int options_from_string_choice(const struct options_table_entry *oe, struct options *oo, const char *name, const char *value, char **cause)`
unsafe fn options_from_string_choice(
    oe: *const options_table_entry,
    oo: *mut options,
    name: &str,
    value: *const u8,
) -> Result<(), CString> {
    unsafe {
        let choice = if value.is_null() {
            let mut choice = options_get_number(oo, name);
            #[expect(clippy::bool_to_int_with_if, reason = "more readable this way")]
            if choice < 2 {
                choice = if choice == 0 { 1 } else { 0 };
            }
            choice
        } else {
            options_find_choice(oe, value)? as i64
        };
        options_set_number(oo, name, choice);
        Ok(())
    }
}

/// C `vendor/tmux/options.c:1126`: `int options_from_string(struct options *oo, const struct options_table_entry *oe, const char *name, const char *value, int append, char **cause)`
pub unsafe fn options_from_string(
    oo: *mut options,
    oe: *const options_table_entry,
    name: &str,
    value: *const u8,
    append: bool,
) -> Result<(), CString> {
    unsafe {
        let new: *const u8;
        let old: *mut u8;
        let key: key_code;

        let type_: options_table_type = if !oe.is_null() {
            if value.is_null()
                && (*oe).type_ != options_table_type::OPTIONS_TABLE_FLAG
                && (*oe).type_ != options_table_type::OPTIONS_TABLE_CHOICE
            {
                return Err(CString::new("empty value").unwrap());
            }
            (*oe).type_
        } else {
            if !name.starts_with('@') {
                return Err(CString::new("bad option name").unwrap());
            }
            options_table_type::OPTIONS_TABLE_STRING
        };

        match type_ {
            options_table_type::OPTIONS_TABLE_STRING => {
                old = xstrdup(options_get_string(oo, name)).as_ptr();
                options_set_string!(oo, name, append, "{}", _s(value));

                new = options_get_string(oo, name);
                if let Err(err) = options_from_string_check(oe, new) {
                    options_set_string!(oo, name, false, "{}", _s(old));
                    free_(old);
                    return Err(err);
                }
                free_(old);
                return Ok(());
            }

            options_table_type::OPTIONS_TABLE_NUMBER => {
                match strtonum(value, (*oe).minimum as i64, (*oe).maximum as i64) {
                    Ok(number) => {
                        options_set_number(oo, name, number);
                        return Ok(());
                    }
                    Err(errstr) => {
                        return Err(CString::new(format!(
                            "value is {}: {}",
                            _s(errstr.as_ptr()),
                            _s(value)
                        ))
                        .unwrap());
                    }
                }
            }

            options_table_type::OPTIONS_TABLE_KEY => {
                key = key_string_lookup_string(value);
                if key == KEYC_UNKNOWN {
                    return Err(CString::new(format!("bad key: {}", _s(value))).unwrap());
                }
                options_set_number(oo, name, key as i64);
                return Ok(());
            }

            options_table_type::OPTIONS_TABLE_COLOUR => {
                let number = colour_fromstring(cstr_to_str(value)) as i64;
                if number == -1 {
                    return Err(CString::new(format!("bad colour: {}", _s(value))).unwrap());
                }
                options_set_number(oo, name, number);
                return Ok(());
            }

            options_table_type::OPTIONS_TABLE_FLAG => {
                return options_from_string_flag(oo, name, value);
            }

            options_table_type::OPTIONS_TABLE_CHOICE => {
                return options_from_string_choice(oe, oo, name, value);
            }

            options_table_type::OPTIONS_TABLE_COMMAND => {}
        }

        Err(CString::new("").unwrap())
    }
}

/// C `vendor/tmux/options.c:1208`: `void options_push_changes(const char *name)`
pub unsafe fn options_push_changes(name: &str) {
    let __func__ = c!("options_push_changes");
    unsafe {
        log_debug!("{}: {}", _s(__func__), name);

        if name == "automatic-rename" {
            for w in rb_foreach(&raw mut WINDOWS).map(NonNull::as_ptr) {
                if (*w).active.is_null() {
                    continue;
                }
                if options_get_number((*w).options, name) != 0 {
                    (*(*w).active).flags |= window_pane_flags::PANE_CHANGED;
                }
            }
        }

        if name == "cursor-colour" {
            for wp in rb_foreach(&raw mut ALL_WINDOW_PANES) {
                window_pane_default_cursor(wp.as_ptr());
            }
        }

        if name == "cursor-style" {
            for wp in rb_foreach(&raw mut ALL_WINDOW_PANES) {
                window_pane_default_cursor(wp.as_ptr());
            }
        }

        if name == "fill-character" {
            for w in rb_foreach(&raw mut WINDOWS) {
                window_set_fill_character(w);
            }
        }

        if name == "key-table" {
            for loop_ in tailq_foreach(&raw mut CLIENTS).map(NonNull::as_ptr) {
                server_client_set_key_table(loop_, null_mut());
            }
        }

        if name == "user-keys" {
            for loop_ in tailq_foreach(&raw mut CLIENTS).map(NonNull::as_ptr) {
                if (*loop_).tty.flags.intersects(tty_flags::TTY_OPENED) {
                    tty_keys_build(&mut (*loop_).tty);
                }
            }
        }

        if name == "status" || name == "status-interval" {
            status_timer_start_all();
        }

        if name == "monitor-silence" {
            alerts_reset_all();
        }

        if name == "window-style" || name == "window-active-style" {
            for wp in rb_foreach(&raw mut ALL_WINDOW_PANES) {
                (*wp.as_ptr()).flags |= window_pane_flags::PANE_STYLECHANGED;
            }
        }

        if name == "pane-colours" {
            for wp in rb_foreach(&raw mut ALL_WINDOW_PANES).map(NonNull::as_ptr) {
                colour_palette_from_option(Some(&mut (*wp).palette), (*wp).options);
            }
        }

        if name == "pane-border-status" {
            for w in rb_foreach(&raw mut WINDOWS) {
                layout_fix_panes(w.as_ptr(), null_mut());
            }
        }

        for s in rb_foreach(&raw mut SESSIONS) {
            status_update_cache(s.as_ptr());
        }

        recalculate_sizes();

        for loop_ in tailq_foreach(&raw mut CLIENTS).map(NonNull::as_ptr) {
            if !(*loop_).session.is_null() {
                server_redraw_client(loop_);
            }
        }
    }
}

// note one difference was that this function previously could avoid allocation on error
/// C `vendor/tmux/options.c:1328`: `int options_remove_or_default(struct options_entry *o, int idx, char **cause)`
pub unsafe fn options_remove_or_default(o: *mut options_entry, idx: i32) -> Result<(), CString> {
    unsafe {
        let oo = (*o).owner;

        if idx == -1 {
            if !(*o).tableentry.is_null()
                && (oo == GLOBAL_OPTIONS || oo == GLOBAL_S_OPTIONS || oo == GLOBAL_W_OPTIONS)
            {
                options_default(oo, (*o).tableentry);
            } else {
                options_remove(o);
            }
        } else {
            options_array_set(o, idx as u32, None, false)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Hand-built table entries so we can exercise number/string/array options
    // without the runtime GLOBAL_OPTIONS trees (which only exist on a running
    // server). Values come from `options_table_entry` in src/lib.rs; the field
    // meanings mirror `struct options_table_entry` in vendor/tmux/tmux.h.
    static NUM_OE: options_table_entry = options_table_entry {
        name: "test-number",
        type_: options_table_type::OPTIONS_TABLE_NUMBER,
        default_num: 42,
        ..options_table_entry::const_default()
    };
    static STR_OE: options_table_entry = options_table_entry {
        name: "test-string",
        type_: options_table_type::OPTIONS_TABLE_STRING,
        default_str: Some("hello"),
        ..options_table_entry::const_default()
    };
    static ARR_OE: options_table_entry = options_table_entry {
        name: "test-array",
        type_: options_table_type::OPTIONS_TABLE_STRING,
        flags: OPTIONS_TABLE_IS_ARRAY,
        ..options_table_entry::const_default()
    };
    // A real, mappable name: options_other_names maps cursor-color ->
    // cursor-colour (vendor/tmux/options-table.c options_other_names[]).
    static COLOUR_OE: options_table_entry = options_table_entry {
        name: "cursor-colour",
        type_: options_table_type::OPTIONS_TABLE_NUMBER,
        default_num: 0,
        ..options_table_entry::const_default()
    };

    // C `options_create(NULL)` (vendor/tmux/options.c:170): a fresh tree is
    // empty and has the given parent.
    #[test]
    fn test_options_create_free_and_parent() {
        unsafe {
            let oo = options_create(null_mut());
            assert!(options_get_parent(oo).is_null());
            // Empty tree: no first entry (RB_MIN of empty tree is NULL).
            assert!(options_first(oo).is_null());

            let child = options_create(oo);
            assert_eq!(options_get_parent(child), oo);

            // options_set_parent just rewrites the parent pointer (options.c:197).
            options_set_parent(&mut *child, null_mut());
            assert!(options_get_parent(child).is_null());

            options_free(child);
            options_free(oo);
        }
    }

    // C `options_set_string`/`options_get_string` (options.c:777/738) for a user
    // option: names starting with '@' need no table entry and are always strings.
    #[test]
    fn test_user_option_string_roundtrip() {
        unsafe {
            let oo = options_create(null_mut());

            let o = options_set_string_(oo, "@foo", false, format_args!("bar"));
            assert!(!o.is_null());
            assert_eq!(options_name(o), "@foo");
            assert!(options_is_string(o));
            assert!(!options_is_array(o));
            assert_eq!(cstr_to_str(options_get_string(oo, "@foo")), "bar");

            // options_get finds it as well and returns the same entry.
            assert_eq!(options_get(&mut *oo, "@foo"), o);

            // Append: user options use an empty separator (options.c:791), so the
            // strings are concatenated directly.
            options_set_string_(oo, "@foo", true, format_args!("baz"));
            assert_eq!(cstr_to_str(options_get_string(oo, "@foo")), "barbaz");

            // Non-append overwrites.
            options_set_string_(oo, "@foo", false, format_args!("qux"));
            assert_eq!(cstr_to_str(options_get_string(oo, "@foo")), "qux");

            options_free(oo);
        }
    }

    // C `options_get` walks up to the parent when the option is not local, while
    // `options_get_only` does not (options.c:215/228).
    #[test]
    fn test_options_get_parent_lookup() {
        unsafe {
            let parent = options_create(null_mut());
            let child = options_create(parent);

            options_set_string_(parent, "@p", false, format_args!("pv"));

            // Not present in the child's own tree.
            assert!(options_get_only(child, "@p").is_null());
            // But visible via the parent chain.
            assert!(!options_get(&mut *child, "@p").is_null());
            assert_eq!(cstr_to_str(options_get_string(child, "@p")), "pv");
            // Missing everywhere -> NULL.
            assert!(options_get(&mut *child, "@missing").is_null());

            options_free(child);
            options_free(parent);
        }
    }

    // C `options_get_only` retries with options_map_name when the direct lookup
    // fails (options.c:215): querying "cursor-color" finds the stored
    // "cursor-colour" entry.
    #[test]
    fn test_options_get_only_name_mapping() {
        unsafe {
            let oo = options_create(null_mut());
            let stored = options_empty(oo, &COLOUR_OE);
            assert_eq!(options_name(stored), "cursor-colour");

            let found = options_get_only(oo, "cursor-color");
            assert_eq!(found, stored);
            assert_eq!(options_name(found), "cursor-colour");

            options_free(oo);
        }
    }

    // C `options_default`/`options_set_number`/`options_get_number`
    // (options.c:257/817/751) for a NUMBER option built from a table entry.
    #[test]
    fn test_number_option_default_and_set() {
        unsafe {
            let oo = options_create(null_mut());
            let o = options_default(oo, &NUM_OE);
            assert!(!o.is_null());
            assert!(!options_is_string(o));
            assert!(!options_is_array(o));
            // default_num is 42.
            assert_eq!(options_get_number(oo, "test-number"), 42);

            // options_set_number finds the existing entry (no parent lookup).
            let o2 = options_set_number(oo, "test-number", -5);
            assert_eq!(o2, o);
            assert_eq!(options_get_number(oo, "test-number"), -5);

            // options_to_string of a scalar number formats the value (options.c:589).
            let s = options_to_string(o, -1, 0);
            assert_eq!(cstr_to_str(s), "-5");
            free_(s);

            options_free(oo);
        }
    }

    // C `options_default` for a STRING option copies default_str; then
    // options_set_string overwrites / appends with the entry's separator
    // (options.c:777, separator NULL -> "").
    #[test]
    fn test_string_option_default_set_append() {
        unsafe {
            let oo = options_create(null_mut());
            let o = options_default(oo, &STR_OE);
            assert!(options_is_string(o));
            assert_eq!(cstr_to_str(options_get_string(oo, "test-string")), "hello");

            options_set_string_(oo, "test-string", false, format_args!("world"));
            assert_eq!(cstr_to_str(options_get_string(oo, "test-string")), "world");

            // Append with a NULL separator concatenates directly.
            options_set_string_(oo, "test-string", true, format_args!("!"));
            assert_eq!(cstr_to_str(options_get_string(oo, "test-string")), "world!");

            options_free(oo);
        }
    }

    // C `options_default_to_string` (options.c:300) renders the default value.
    #[test]
    fn test_default_to_string() {
        unsafe {
            let n = options_default_to_string(&NUM_OE);
            assert_eq!(cstr_to_str(n.as_ptr()), "42");
            free_(n.as_ptr());

            let s = options_default_to_string(&STR_OE);
            assert_eq!(cstr_to_str(s.as_ptr()), "hello");
            free_(s.as_ptr());
        }
    }

    // C array handling: options_empty inits the array, options_array_set/get,
    // options_array_first/next/item_index/item_value, options_to_string
    // (options.c:243, 433, 420, 551-575, 589).
    #[test]
    fn test_array_option() {
        unsafe {
            let oo = options_create(null_mut());
            let o = options_empty(oo, &ARR_OE);
            assert!(options_is_array(o));

            // Empty array: no first item, get out of range -> NULL,
            // to_string -> "".
            assert!(options_array_first(o).is_null());
            assert!(options_array_get(o, 0).is_null());
            let empty = options_to_string(o, -1, 0);
            assert_eq!(cstr_to_str(empty), "");
            free_(empty);

            assert!(options_array_set(o, 0, Some("a"), false).is_ok());
            assert!(options_array_set(o, 1, Some("b"), false).is_ok());

            let v0 = options_array_get(o, 0);
            assert!(!v0.is_null());
            assert_eq!(cstr_to_str((*v0).string), "a");

            // Iteration is ordered by index.
            let first = options_array_first(o);
            assert_eq!(options_array_item_index(first), 0);
            assert_eq!(cstr_to_str((*options_array_item_value(first)).string), "a");
            let second = options_array_next(first);
            assert_eq!(options_array_item_index(second), 1);
            assert!(options_array_next(second).is_null());

            // Whole-array to_string joins with spaces.
            let joined = options_to_string(o, -1, 0);
            assert_eq!(cstr_to_str(joined), "a b");
            free_(joined);

            // Single-index to_string.
            let one = options_to_string(o, 1, 0);
            assert_eq!(cstr_to_str(one), "b");
            free_(one);

            // Setting a value of None removes that index (options.c:448).
            assert!(options_array_set(o, 0, None, false).is_ok());
            assert!(options_array_get(o, 0).is_null());
            assert_eq!(options_array_item_index(options_array_first(o)), 1);

            options_array_clear(o);
            assert!(options_array_first(o).is_null());

            options_free(oo);
        }
    }

    // C `options_array_assign` (options.c:510): with a NULL separator the value
    // is split on the default " ," set into successive indices.
    #[test]
    fn test_array_assign() {
        unsafe {
            let oo = options_create(null_mut());
            let o = options_empty(oo, &ARR_OE);

            assert!(options_array_assign(o, "x,y z").is_ok());
            let joined = options_to_string(o, -1, 0);
            assert_eq!(cstr_to_str(joined), "x y z");
            free_(joined);

            // Empty string assigns nothing.
            let oo2 = options_create(null_mut());
            let o2 = options_empty(oo2, &ARR_OE);
            assert!(options_array_assign(o2, "").is_ok());
            assert!(options_array_first(o2).is_null());

            options_free(oo2);
            options_free(oo);
        }
    }

    // options_array_set on a non-array entry is an error (options.c:442).
    #[test]
    fn test_array_set_wrong_type() {
        unsafe {
            let oo = options_create(null_mut());
            let o = options_default(oo, &STR_OE); // scalar string, not an array
            assert!(options_array_set(o, 0, Some("x"), false).is_err());
            // options_array_get / first on a non-array return NULL.
            assert!(options_array_get(o, 0).is_null());
            assert!(options_array_first(o).is_null());
            options_free(oo);
        }
    }

    // C `options_remove_or_default` (options.c:1327): for a non-global tree with
    // idx == -1 the entry is removed entirely.
    #[test]
    fn test_remove_or_default_removes() {
        unsafe {
            let oo = options_create(null_mut());
            let o = options_set_string_(oo, "@foo", false, format_args!("bar"));
            assert!(!options_get_only(oo, "@foo").is_null());

            assert!(options_remove_or_default(o, -1).is_ok());
            assert!(options_get_only(oo, "@foo").is_null());

            options_free(oo);
        }
    }

    // C `options_first`/`options_next` walk the tree in name order
    // (options.c:203/209, compare via options_cmp = strcmp).
    #[test]
    fn test_iteration_order() {
        unsafe {
            let oo = options_create(null_mut());
            options_set_string_(oo, "@b", false, format_args!("1"));
            options_set_string_(oo, "@a", false, format_args!("2"));
            options_set_string_(oo, "@c", false, format_args!("3"));

            let mut o = options_first(oo);
            assert_eq!(options_name(o), "@a");
            o = options_next(o);
            assert_eq!(options_name(o), "@b");
            o = options_next(o);
            assert_eq!(options_name(o), "@c");
            assert!(options_next(o).is_null());

            options_free(oo);
        }
    }

    // C `options_parse` (options.c:623): "name" -> (name, -1); "name[idx]"
    // -> (name, idx); malformed forms -> NULL.
    #[test]
    fn test_options_parse() {
        assert_eq!(options_parse("foo"), Some(("foo".to_string(), -1)));
        assert_eq!(options_parse("foo[3]"), Some(("foo".to_string(), 3)));
        assert_eq!(options_parse("foo[10]"), Some(("foo".to_string(), 10)));
        assert_eq!(options_parse("@user[2]"), Some(("@user".to_string(), 2)));
        // No '[' -> whole name, idx -1 (even a trailing ']').
        assert_eq!(options_parse("foo]"), Some(("foo]".to_string(), -1)));

        // Empty name -> None.
        assert_eq!(options_parse(""), None);
        // Empty brackets: char before ']' is not a digit.
        assert_eq!(options_parse("foo[]"), None);
        // Non-digit index.
        assert_eq!(options_parse("foo[bar]"), None);
        // Trailing junk after ']'.
        assert_eq!(options_parse("foo[3]x"), None);
    }

    // C `options_match` (options.c:677): '@' names pass through; real names are
    // resolved (with options_map_name) against the options table; unknown names
    // return NULL. idx/ambiguous are written through the out-pointers.
    #[test]
    fn test_options_match() {
        unsafe {
            let mut idx = 99i32;
            let mut amb = 99i32;

            // User option: returned verbatim, unambiguous, index parsed.
            let m = options_match("@foo[5]", &mut idx, &mut amb);
            assert_eq!(m.as_deref(), Some("@foo"));
            assert_eq!(idx, 5);
            assert_eq!(amb, 0);

            // Exact real option name. NOTE: like C options_match (options.c:677),
            // *ambiguous is only written on the not-found / ambiguous branches, so
            // we do not assert it on the success path.
            idx = 99;
            let m = options_match("buffer-limit", &mut idx, &mut amb);
            assert_eq!(m.as_deref(), Some("buffer-limit"));
            assert_eq!(idx, -1);

            // Mapped alias resolves to the canonical table name.
            let m = options_match("cursor-color", &mut idx, &mut amb);
            assert_eq!(m.as_deref(), Some("cursor-colour"));

            // Completely unknown option: NULL, not ambiguous.
            amb = 99;
            let m = options_match("zzz-not-an-option", &mut idx, &mut amb);
            assert_eq!(m, None);
            assert_eq!(amb, 0);

            // Malformed name (empty) parses to None before any table lookup.
            let m = options_match("", &mut idx, &mut amb);
            assert_eq!(m, None);
        }
    }

    // C `options_parse_get` (options.c:648): parses name[idx], writing idx, and
    // looks up the entry (only=1 restricts to the local tree).
    #[test]
    fn test_options_parse_get() {
        unsafe {
            let oo = options_create(null_mut());
            options_set_string_(oo, "@foo", false, format_args!("bar"));

            let mut idx = 99i32;
            let o = options_parse_get(oo, "@foo[7]", &mut idx, 1);
            assert!(!o.is_null());
            assert_eq!(idx, 7);
            assert_eq!(options_name(o), "@foo");

            // Missing option -> NULL entry, idx still written.
            idx = 99;
            let o = options_parse_get(oo, "@nope", &mut idx, 1);
            assert!(o.is_null());
            assert_eq!(idx, -1);

            options_free(oo);
        }
    }

    // Additional hand-built table entries for the number/flag/choice/colour/key
    // option types (mirroring `struct options_table_entry`, vendor/tmux/tmux.h).
    static NUMR_OE: options_table_entry = options_table_entry {
        name: "test-num-range",
        type_: options_table_type::OPTIONS_TABLE_NUMBER,
        minimum: 0,
        maximum: 100,
        default_num: 10,
        ..options_table_entry::const_default()
    };
    static FLAG_OE: options_table_entry = options_table_entry {
        name: "test-flag",
        type_: options_table_type::OPTIONS_TABLE_FLAG,
        default_num: 1,
        ..options_table_entry::const_default()
    };
    static CHOICE_OE: options_table_entry = options_table_entry {
        name: "test-choice",
        type_: options_table_type::OPTIONS_TABLE_CHOICE,
        choices: &["off", "on", "both"],
        default_num: 2,
        ..options_table_entry::const_default()
    };
    static CLR_OE: options_table_entry = options_table_entry {
        name: "test-colour",
        type_: options_table_type::OPTIONS_TABLE_COLOUR,
        default_num: 1, // colour 1 == "red"
        ..options_table_entry::const_default()
    };
    static KEY_OE: options_table_entry = options_table_entry {
        name: "test-key",
        type_: options_table_type::OPTIONS_TABLE_KEY,
        ..options_table_entry::const_default()
    };

    // C `options_default` (options.c:258): a CHOICE default stores default_num as
    // the value; options_to_string / options_default_to_string then render it via
    // choices[number] (options.c:133/301).
    #[test]
    fn test_choice_option_default_and_to_string() {
        unsafe {
            let oo = options_create(null_mut());
            let o = options_default(oo, &CHOICE_OE);
            assert!(!options_is_string(o));
            // default_num 2 -> choices[2] == "both".
            let s = options_to_string(o, -1, 0);
            assert_eq!(cstr_to_str(s), "both");
            free_(s);

            let d = options_default_to_string(&CHOICE_OE);
            assert_eq!(cstr_to_str(d.as_ptr()), "both");
            free_(d.as_ptr());

            options_free(oo);
        }
    }

    // C `options_find_choice` (options.c:1088): returns the index of the matching
    // choice string, or an error for an unknown value.
    #[test]
    fn test_find_choice() {
        unsafe {
            assert_eq!(options_find_choice(&CHOICE_OE, c!("off")).unwrap(), 0);
            assert_eq!(options_find_choice(&CHOICE_OE, c!("on")).unwrap(), 1);
            assert_eq!(options_find_choice(&CHOICE_OE, c!("both")).unwrap(), 2);
            assert!(options_find_choice(&CHOICE_OE, c!("nope")).is_err());
        }
    }

    // C `options_from_string` CHOICE branch (options.c:1107/1200): a value is
    // resolved through options_find_choice and stored as its index.
    #[test]
    fn test_from_string_choice() {
        unsafe {
            let oo = options_create(null_mut());
            options_default(oo, &CHOICE_OE);

            assert!(options_from_string(oo, &CHOICE_OE, "test-choice", c!("on"), false).is_ok());
            assert_eq!(options_get_number_(oo, "test-choice"), 1);

            // Unknown choice -> error, value unchanged.
            assert!(options_from_string(oo, &CHOICE_OE, "test-choice", c!("bogus"), false).is_err());
            assert_eq!(options_get_number_(oo, "test-choice"), 1);

            options_free(oo);
        }
    }

    // C `options_default` FLAG branch and options_value_to_string FLAG rendering
    // (options.c:133): non-numeric renders "on"/"off", numeric renders the raw
    // integer.
    #[test]
    fn test_flag_option_default_and_render() {
        unsafe {
            let oo = options_create(null_mut());
            let o = options_default(oo, &FLAG_OE);
            // default_num 1 -> "on" (non-numeric) or "1" (numeric).
            let on = options_to_string(o, -1, 0);
            assert_eq!(cstr_to_str(on), "on");
            free_(on);
            let num = options_to_string(o, -1, 1);
            assert_eq!(cstr_to_str(num), "1");
            free_(num);

            let d = options_default_to_string(&FLAG_OE);
            assert_eq!(cstr_to_str(d.as_ptr()), "on");
            free_(d.as_ptr());

            options_set_number(oo, "test-flag", 0);
            let off = options_to_string(o, -1, 0);
            assert_eq!(cstr_to_str(off), "off");
            free_(off);

            options_free(oo);
        }
    }

    // C `options_from_string_flag` (options.c:1064): "on"/"yes"/"1" -> true,
    // "off"/"no"/"0" -> false (case-insensitive), anything else is an error.
    #[test]
    fn test_from_string_flag() {
        unsafe {
            let oo = options_create(null_mut());
            options_default(oo, &FLAG_OE);

            for on in ["on", "YES", "1"] {
                let mut v = on.to_string();
                v.push('\0');
                assert!(
                    options_from_string(oo, &FLAG_OE, "test-flag", v.as_bytes().as_ptr(), false)
                        .is_ok()
                );
                assert_eq!(options_get_number_(oo, "test-flag"), 1);
            }
            for off in ["off", "No", "0"] {
                let mut v = off.to_string();
                v.push('\0');
                assert!(
                    options_from_string(oo, &FLAG_OE, "test-flag", v.as_bytes().as_ptr(), false)
                        .is_ok()
                );
                assert_eq!(options_get_number_(oo, "test-flag"), 0);
            }
            // Garbage value is rejected.
            assert!(options_from_string(oo, &FLAG_OE, "test-flag", c!("maybe"), false).is_err());

            options_free(oo);
        }
    }

    // C `options_from_string` NUMBER branch (options.c:1150): strtonum enforces
    // the [minimum, maximum] range from the table entry.
    #[test]
    fn test_from_string_number_range() {
        unsafe {
            let oo = options_create(null_mut());
            options_default(oo, &NUMR_OE);

            assert!(options_from_string(oo, &NUMR_OE, "test-num-range", c!("50"), false).is_ok());
            assert_eq!(options_get_number_(oo, "test-num-range"), 50);

            // Above maximum (100) -> error, value unchanged.
            assert!(options_from_string(oo, &NUMR_OE, "test-num-range", c!("999"), false).is_err());
            assert_eq!(options_get_number_(oo, "test-num-range"), 50);
            // Non-numeric -> error.
            assert!(options_from_string(oo, &NUMR_OE, "test-num-range", c!("abc"), false).is_err());
            assert_eq!(options_get_number_(oo, "test-num-range"), 50);

            options_free(oo);
        }
    }

    // C `options_default`/`options_value_to_string` COLOUR (options.c:133): the
    // number is a colour code rendered by colour_tostring; colour 1 == "red".
    #[test]
    fn test_colour_option_default_and_render() {
        unsafe {
            let oo = options_create(null_mut());
            let o = options_default(oo, &CLR_OE);
            let s = options_to_string(o, -1, 0);
            assert_eq!(cstr_to_str(s), "red");
            free_(s);

            let d = options_default_to_string(&CLR_OE);
            assert_eq!(cstr_to_str(d.as_ptr()), "red");
            free_(d.as_ptr());

            options_free(oo);
        }
    }

    // C `options_from_string` COLOUR branch (options.c:1175): value parsed by
    // colour_fromstring; an unknown colour name is rejected.
    #[test]
    fn test_from_string_colour() {
        unsafe {
            let oo = options_create(null_mut());
            options_default(oo, &CLR_OE);

            // "green" == colour 2; round-trips through to_string.
            assert!(options_from_string(oo, &CLR_OE, "test-colour", c!("green"), false).is_ok());
            let o = options_get_only(oo, "test-colour");
            let s = options_to_string(o, -1, 0);
            assert_eq!(cstr_to_str(s), "green");
            free_(s);

            // colour_fromstring returns -1 for junk -> error.
            assert!(
                options_from_string(oo, &CLR_OE, "test-colour", c!("zznotacolour"), false).is_err()
            );

            options_free(oo);
        }
    }

    // C `options_from_string` KEY branch (options.c:1160): an unparseable key
    // string yields KEYC_UNKNOWN and is rejected as a "bad key".
    #[test]
    fn test_from_string_bad_key() {
        unsafe {
            let oo = options_create(null_mut());
            options_default(oo, &KEY_OE);
            assert!(
                options_from_string(oo, &KEY_OE, "test-key", c!("not-a-real-key-zzz"), false)
                    .is_err()
            );
            options_free(oo);
        }
    }

    // C `options_from_string` STRING branch for a user option (options.c:1140):
    // oe == NULL is allowed only for '@' names. The STRING branch reads the
    // existing value first (options_get_string, which fatalx-aborts on a missing
    // option — options.c:738), so the entry must already exist; create it empty.
    #[test]
    fn test_from_string_user_string() {
        unsafe {
            let oo = options_create(null_mut());
            options_set_string_(oo, "@u", false, format_args!(""));
            assert!(options_from_string(oo, null(), "@u", c!("hello"), false).is_ok());
            assert_eq!(cstr_to_str(options_get_string(oo, "@u")), "hello");

            // Append concatenates (user options use an empty separator).
            assert!(options_from_string(oo, null(), "@u", c!("!"), true).is_ok());
            assert_eq!(cstr_to_str(options_get_string(oo, "@u")), "hello!");

            // Non-'@' name with a NULL table entry is rejected.
            assert!(options_from_string(oo, null(), "plain", c!("x"), false).is_err());

            options_free(oo);
        }
    }

    // C `options_get_number` (options.c:751) via the public const wrapper: reads a
    // NUMBER value, walking to the parent when the option is not local.
    #[test]
    fn test_get_number_parent_lookup() {
        unsafe {
            let parent = options_create(null_mut());
            options_default(parent, &NUM_OE);
            options_set_number(parent, "test-number", 7);

            let child = options_create(parent);
            // Not present locally, but visible via the parent chain.
            assert!(options_get_only(child, "test-number").is_null());
            assert_eq!(options_get_number_(child, "test-number"), 7);

            options_free(child);
            options_free(parent);
        }
    }

    // C `options_remove_or_default` with idx >= 0 (options.c:1328): removes only
    // that array index, leaving the rest intact.
    #[test]
    fn test_remove_or_default_array_index() {
        unsafe {
            let oo = options_create(null_mut());
            let o = options_empty(oo, &ARR_OE);
            assert!(options_array_set(o, 0, Some("a"), false).is_ok());
            assert!(options_array_set(o, 1, Some("b"), false).is_ok());

            assert!(options_remove_or_default(o, 0).is_ok());
            assert!(options_array_get(o, 0).is_null());
            // Index 1 survives.
            assert_eq!(cstr_to_str((*options_array_get(o, 1)).string), "b");

            options_free(oo);
        }
    }

    // C `options_array_set` with append (options.c:433): when the entry has no
    // separator the appended value is concatenated directly onto the existing
    // element.
    #[test]
    fn test_array_set_append() {
        unsafe {
            let oo = options_create(null_mut());
            let o = options_empty(oo, &ARR_OE);
            assert!(options_array_set(o, 0, Some("foo"), false).is_ok());
            // append=true with a NULL separator -> "foobar".
            assert!(options_array_set(o, 0, Some("bar"), true).is_ok());
            assert_eq!(cstr_to_str((*options_array_get(o, 0)).string), "foobar");
            options_free(oo);
        }
    }

    // C `options_empty` on an array entry re-inits the RB tree so a fresh entry
    // is empty and iterable (options.c:244).
    #[test]
    fn test_empty_array_is_iterable() {
        unsafe {
            let oo = options_create(null_mut());
            let o = options_empty(oo, &ARR_OE);
            assert!(options_is_array(o));
            assert!(options_array_first(o).is_null());
            // Setting past index 0 works and preserves index ordering.
            assert!(options_array_set(o, 5, Some("z"), false).is_ok());
            assert_eq!(options_array_item_index(options_array_first(o)), 5);
            options_free(oo);
        }
    }

    // An array whose table entry has a default_str but no default_arr is filled
    // by splitting the default string (options.c:258, default branch calls
    // options_array_assign).
    static ARR_DEF_OE: options_table_entry = options_table_entry {
        name: "test-array-def",
        type_: options_table_type::OPTIONS_TABLE_STRING,
        flags: OPTIONS_TABLE_IS_ARRAY,
        default_str: Some("a b c"),
        ..options_table_entry::const_default()
    };
    #[test]
    fn test_array_default_from_string() {
        unsafe {
            let oo = options_create(null_mut());
            let o = options_default(oo, &ARR_DEF_OE);
            assert!(options_is_array(o));
            let joined = options_to_string(o, -1, 0);
            assert_eq!(cstr_to_str(joined), "a b c");
            free_(joined);
            // Three distinct indices 0,1,2.
            assert_eq!(options_array_item_index(options_array_first(o)), 0);
            options_free(oo);
        }
    }

    // C `options_to_string` with an out-of-range array index returns the empty
    // string (options.c:589, options_array_item is NULL).
    #[test]
    fn test_to_string_array_index_out_of_range() {
        unsafe {
            let oo = options_create(null_mut());
            let o = options_empty(oo, &ARR_OE);
            assert!(options_array_set(o, 0, Some("only"), false).is_ok());
            // Present index renders its value.
            let hit = options_to_string(o, 0, 0);
            assert_eq!(cstr_to_str(hit), "only");
            free_(hit);
            // Missing index renders "".
            let miss = options_to_string(o, 9, 0);
            assert_eq!(cstr_to_str(miss), "");
            free_(miss);
            options_free(oo);
        }
    }

    // C `options_default_to_string` renders a COLOUR default via colour_tostring
    // (options.c:301); colour 2 == "green".
    #[test]
    fn test_default_to_string_colour_green() {
        unsafe {
            let oe = options_table_entry {
                name: "c2",
                type_: options_table_type::OPTIONS_TABLE_COLOUR,
                default_num: 2,
                ..options_table_entry::const_default()
            };
            let d = options_default_to_string(&oe);
            assert_eq!(cstr_to_str(d.as_ptr()), "green");
            free_(d.as_ptr());
        }
    }
}
