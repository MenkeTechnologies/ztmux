// Copyright (c) 2010 Nicholas Marriott <nicholas.marriott@gmail.com>
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
use crate::*;

pub type args_values = tailq_head<args_value>;

const ARGS_ENTRY_OPTIONAL_VALUE: c_int = 1;
#[repr(C)]
pub struct args_entry {
    pub flag: c_uchar,
    pub values: args_values,
    pub count: c_uint,

    pub flags: c_int,

    pub entry: rb_entry<args_entry>,
}

#[repr(C)]
pub struct args {
    pub tree: args_tree,
    pub count: u32,
    pub values: *mut args_value,
}

#[repr(C)]
pub struct args_command_state<'a> {
    pub cmdlist: *mut cmd_list,
    pub cmd: *mut u8,
    pub pi: cmd_parse_input<'a>,
}

RB_GENERATE!(args_tree, args_entry, entry, discr_entry, args_cmp);

/// C `vendor/tmux/arguments.c:68`: `static int args_cmp(struct args_entry *a1, struct args_entry *a2)`
fn args_cmp(a1: &args_entry, a2: &args_entry) -> cmp::Ordering {
    a1.flag.cmp(&a2.flag)
}

/// C `vendor/tmux/arguments.c:75`: `static struct args_entry *args_find(struct args *args, u_char flag)`
pub unsafe fn args_find(args: *mut args, flag: c_uchar) -> *mut args_entry {
    unsafe {
        let mut entry: args_entry = args_entry { flag, ..zeroed() };

        rb_find(&raw mut (*args).tree, &raw mut entry)
    }
}

/// C `vendor/tmux/arguments.c:85`: `static void args_copy_value(struct args_value *to, struct args_value *from)`
pub unsafe fn args_copy_value(to: *mut args_value, from: *const args_value) {
    unsafe {
        (*to).type_ = (*from).type_;
        match (*from).type_ {
            args_type::ARGS_NONE => (),
            args_type::ARGS_COMMANDS => {
                (*to).union_.cmdlist = (*from).union_.cmdlist;
                (*(*to).union_.cmdlist).references += 1;
            }
            args_type::ARGS_STRING => {
                (*to).union_.string = xstrdup((*from).union_.string).cast().as_ptr();
            }
        }
    }
}

/// C `vendor/tmux/arguments.c:103`: `static const char *args_type_to_string (enum args_type type)`
pub fn args_type_to_string(type_: args_type) -> &'static str {
    match type_ {
        args_type::ARGS_NONE => "NONE",
        args_type::ARGS_STRING => "STRING",
        args_type::ARGS_COMMANDS => "COMMANDS",
    }
}

/// C `vendor/tmux/arguments.c:119`: `static const char *args_value_as_string(struct args_value *value)`
impl args_value {
    #[inline]
    pub(crate) fn cached_ptr(&self) -> *const u8 {
        match &self.cached {
            Some(c) => c.as_ptr().cast(),
            None => std::ptr::null(),
        }
    }
}

pub unsafe fn args_value_as_string(value: *mut args_value) -> *const u8 {
    unsafe {
        match (*value).type_ {
            args_type::ARGS_NONE => c!(""),
            args_type::ARGS_STRING => (*value).union_.string,
            args_type::ARGS_COMMANDS => {
                if (*value).cached.is_none() {
                    (*value).cached = Some(std::ffi::CString::from_raw(
                        cmd_list_print(&*(*value).union_.cmdlist, 0).cast(),
                    ));
                }
                (*value).cached_ptr()
            }
        }
    }
}

impl args {
    fn create() -> Box<Self> {
        Box::new(Self {
            tree: rb_head::rb_init(),
            count: 0,
            values: null_mut(),
        })
    }
}

/// C `vendor/tmux/arguments.c:136`: `struct args *args_create(void)`
pub fn args_create<'a>() -> &'a mut args {
    Box::leak(args::create())
}

/// C `vendor/tmux/arguments.c:147`: `static int args_parse_flag_argument(struct args_value *values, u_int count, char **cause, struct args *args, u_int *i, const char *string, int flag, int optional_argument)`
pub unsafe fn args_parse_flag_argument(
    values: *const args_value,
    count: u32,
    cause: *mut *mut u8,
    args: *mut args,
    i: *mut u32,
    string: *const u8,
    flag: i32,
    optional_argument: bool,
) -> i32 {
    let argument: *const args_value;
    let new: *mut args_value;
    unsafe {
        'out: {
            new = xcalloc(1, size_of::<args_value>()).cast().as_ptr();

            if *string != b'\0' {
                (*new).type_ = args_type::ARGS_STRING;
                (*new).union_.string = xstrdup(string).cast().as_ptr();
                break 'out;
            }

            if *i == count {
                argument = null_mut();
            } else {
                argument = values.add(*i as usize);
                if (*argument).type_ != args_type::ARGS_STRING {
                    *cause = format_nul!("-{} argument must be a string", flag as u8 as char);
                    args_free_value(new);
                    free(new as _);
                    return -1;
                }
            }

            if argument.is_null() {
                args_free_value(new);
                free(new as _);
                if optional_argument {
                    log_debug!("{}: -{} (optional)", "args_parse_flag_argument", flag);
                    args_set(args, flag as c_uchar, null_mut(), ARGS_ENTRY_OPTIONAL_VALUE);
                    return 0; /* either - or end */
                }
                *cause = format_nul!("-{} expects an argument", flag as u8 as char);
                return -1;
            }

            // C `vendor/tmux/arguments.c:184-192`: for an optional-argument flag,
            // if the following token looks like another flag (begins with '-'
            // and is followed by '-' or an alpha), do NOT swallow it as the
            // value; record this flag with an optional (empty) value and leave
            // the following token to be parsed as its own flag.
            if optional_argument && (*argument).type_ == args_type::ARGS_STRING {
                let as_ = (*argument).union_.string;
                if *as_ == b'-'
                    && (*as_.add(1) == b'-' || (*as_.add(1)).is_ascii_alphabetic())
                {
                    args_free_value(new);
                    free(new as _);
                    log_debug!("{}: -{} (optional)", "args_parse_flag_argument", flag);
                    args_set(args, flag as c_uchar, null_mut(), ARGS_ENTRY_OPTIONAL_VALUE);
                    return 0;
                }
            }

            args_copy_value(new, argument);
            (*i) += 1;

            break 'out;
        }
        // out:
        let s = args_value_as_string(new);
        log_debug!("{}: -{} = {}", "args_parse_flag_argument", flag, _s(s));
        args_set(args, flag as c_uchar, new, 0);
    }

    0
}

#[expect(clippy::needless_borrow, reason = "false positive")]
/// C `vendor/tmux/arguments.c:207`: `static int args_parse_flags(const struct args_parse *parse, struct args_value *values, u_int count, char **cause, struct args *args, u_int *i)`
pub unsafe fn args_parse_flags(
    parse: *const args_parse,
    values: *const args_value,
    count: u32,
    cause: *mut *mut u8,
    args: *mut args,
    i: *mut u32,
) -> i32 {
    let __func__ = "args_parse_flags";
    unsafe {
        let value = values.add(*i as usize);
        if (*value).type_ != args_type::ARGS_STRING {
            return 1;
        }

        let mut string = (*value).union_.string;
        log_debug!("{}: next {}", __func__, _s(string));
        if ({
            let tmp = *string != b'-';
            string = string.add(1);
            tmp
        }) || *string == b'\0'
        {
            return 1;
        }
        (*i) += 1;
        if *string == b'-' && *string.add(1) == b'\0' {
            return 1;
        }

        loop {
            let flag = *string as c_uchar;
            string = string.add(1);
            if flag == b'\0' {
                return 0;
            }
            if flag == b'?' {
                return -1;
            }
            if !flag.is_ascii_alphanumeric() {
                *cause = format_nul!("invalid flag -{}", flag as char);
                return -1;
            }

            let Some(found) = (*parse).template.bytes().position(|ch| ch == flag) else {
                *cause = format_nul!("unknown flag -{}", flag as char);
                return -1;
            };
            if found + 1 >= (&(*parse).template).len() || (*parse).template.as_bytes()[found + 1] != b':' {
                log_debug!("{}: -{}", __func__, flag as char);
                args_set(args, flag, null_mut(), 0);
                continue;
            }
            let optional_argument = found + 2 < (&(*parse).template).len() && (*parse).template.as_bytes()[found + 2] == b':';
            return args_parse_flag_argument(
                values,
                count,
                cause,
                args,
                i,
                string,
                flag as i32,
                optional_argument,
            );
        }
    }
}

/// Parse arguments into a new argument set.
/// C `vendor/tmux/arguments.c:256`: `struct args *args_parse(const struct args_parse *parse, struct args_value *values, u_int count, char **cause)`
pub unsafe fn args_parse(
    parse: *const args_parse,
    values: *mut args_value,
    count: u32,
    cause: *mut *mut u8,
) -> *mut args {
    let __func__ = "args_parse";
    unsafe {
        let mut type_: args_parse_type;

        if count == 0 {
            return args_create();
        }

        let args = args_create();

        let mut i: u32 = 1;
        while i < count {
            let stop = args_parse_flags(parse, values, count, cause, args, &raw mut i);
            if stop == -1 {
                args_free(args);
                return null_mut();
            }
            if stop == 1 {
                break;
            }
        }
        log_debug!("{}: flags end at {} of {}", __func__, i, count);
        if i != count {
            while i < count {
                let value = values.add(i as usize);

                let s = args_value_as_string(value);
                log_debug!(
                    "{}: {} = {} (type {})",
                    __func__,
                    i,
                    _s(s),
                    args_type_to_string((*value).type_),
                );

                if let Some(cb) = (*parse).cb {
                    type_ = cb(args, args.count, cause);
                    if type_ == args_parse_type::ARGS_PARSE_INVALID {
                        args_free(args);
                        return null_mut();
                    }
                } else {
                    type_ = args_parse_type::ARGS_PARSE_STRING;
                }

                args.values = xrecallocarray(
                    args.values.cast(),
                    args.count as usize,
                    args.count as usize + 1,
                    size_of::<args_value>(),
                )
                .cast()
                .as_ptr();
                let new = args.values.add(args.count as usize);
                args.count += 1;

                match type_ {
                    args_parse_type::ARGS_PARSE_INVALID => fatalx("unexpected argument type"),
                    args_parse_type::ARGS_PARSE_STRING => {
                        if (*value).type_ != args_type::ARGS_STRING {
                            *cause = format_nul!("argument {} must be \"string\"", args.count);
                            args_free(args);
                            return null_mut();
                        }
                        args_copy_value(new, value);
                    }
                    args_parse_type::ARGS_PARSE_COMMANDS_OR_STRING => args_copy_value(new, value),
                    args_parse_type::ARGS_PARSE_COMMANDS => {
                        if (*value).type_ != args_type::ARGS_COMMANDS {
                            *cause = format_nul!("argument {} must be {{ commands }}", args.count,);
                            args_free(args);
                            return null_mut();
                        }
                        args_copy_value(new, value);
                    }
                }
                i += 1;
            }
        }

        if (*parse).lower != -1 && args.count < (*parse).lower as u32 {
            *cause = format_nul!("too few arguments (need at least {})", (*parse).lower);
            args_free(args);
            return null_mut();
        }
        if (*parse).upper != -1 && args.count > (*parse).upper as u32 {
            *cause = format_nul!("too many arguments (need at most {})", (*parse).upper);
            args_free(args);
            return null_mut();
        }
        args
    }
}

/// C `vendor/tmux/arguments.c:350`: `static void args_copy_copy_value(struct args_value *to, struct args_value *from, int argc, char **argv)`
pub unsafe fn args_copy_copy_value(
    to: *mut args_value,
    from: *const args_value,
    argc: i32,
    argv: *mut *mut u8,
) {
    unsafe {
        (*to).type_ = (*from).type_;
        match (*from).type_ {
            args_type::ARGS_NONE => (),
            args_type::ARGS_STRING => {
                let mut expanded = xstrdup((*from).union_.string).as_ptr();
                for i in 0..argc {
                    let s =
                        cmd_template_replace(expanded, cstr_to_str_(*argv.add(i as usize)), i + 1);
                    free_(expanded);
                    expanded = s;
                }
                (*to).union_.string = expanded;
            }
            args_type::ARGS_COMMANDS => {
                (*to).union_.cmdlist = cmd_list_copy(&*(*from).union_.cmdlist, argc, argv);
            }
        }
    }
}

/// Copy an arguments set.
/// C `vendor/tmux/arguments.c:377`: `struct args *args_copy(struct args *args, int argc, char **argv)`
pub unsafe fn args_copy(args: *mut args, argc: i32, argv: *mut *mut u8) -> *mut args {
    let __func__ = "args_copy";
    unsafe {
        cmd_log_argv!(argc, argv, "{__func__}");

        let new_args = args_create();
        for entry in rb_foreach(&raw mut (*args).tree).map(NonNull::as_ptr) {
            if tailq_empty(&raw mut (*entry).values) {
                for _ in 0..(*entry).count {
                    args_set(new_args, (*entry).flag, null_mut(), 0);
                }
                continue;
            }
            for value in tailq_foreach(&raw mut (*entry).values) {
                let new_value = xcalloc1();
                args_copy_copy_value(new_value, value.as_ptr(), argc, argv);
                args_set(new_args, (*entry).flag, new_value, 0);
            }
        }
        if (*args).count == 0 {
            return new_args;
        }
        new_args.count = (*args).count;
        new_args.values = xcalloc_((*args).count as usize).as_ptr();
        for i in 0..(*args).count {
            let new_value = new_args.values.add(i as usize);
            args_copy_copy_value(new_value, (*args).values.add(i as usize), argc, argv);
        }

        new_args
    }
}

/// C `vendor/tmux/arguments.c:412`: `void args_free_value(struct args_value *value)`
pub unsafe fn args_free_value(value: *mut args_value) {
    unsafe {
        match (*value).type_ {
            args_type::ARGS_NONE => (),
            args_type::ARGS_STRING => free_((*value).union_.string),
            args_type::ARGS_COMMANDS => cmd_list_free((*value).union_.cmdlist),
        }
        (*value).cached = None;
    }
}

/// C `vendor/tmux/arguments.c:429`: `void args_free_values(struct args_value *values, u_int count)`
pub unsafe fn args_free_values(values: *mut args_value, count: u32) {
    unsafe {
        for i in 0..count {
            args_free_value(values.add(i as usize));
        }
    }
}

/// C `vendor/tmux/arguments.c:439`: `void args_free(struct args *args)`
pub unsafe fn args_free(args: *mut args) {
    unsafe {
        args_free_values((*args).values, (*args).count);
        free_((*args).values);

        for entry in rb_foreach(&raw mut (*args).tree).map(NonNull::as_ptr) {
            rb_remove(&raw mut (*args).tree, entry);
            for value in tailq_foreach(&raw mut (*entry).values).map(NonNull::as_ptr) {
                tailq_remove(&raw mut (*entry).values, value);
                args_free_value(value);
                free_(value);
            }
            free_(entry);
        }

        free_(args);
    }
}

/// C `vendor/tmux/arguments.c:464`: `void args_to_vector(struct args *args, int *argc, char ***argv)`
pub unsafe fn args_to_vector(args: *const args, argc: *mut i32, argv: *mut *mut *mut u8) {
    unsafe {
        *argc = 0;
        *argv = null_mut();

        for i in 0..(*args).count {
            match (*(*args).values.add(i as usize)).type_ {
                args_type::ARGS_NONE => (),
                args_type::ARGS_STRING => {
                    cmd_append_argv(argc, argv, (*(*args).values.add(i as usize)).union_.string);
                }
                args_type::ARGS_COMMANDS => {
                    let s =
                        cmd_list_print(&*(*(*args).values.add(i as usize)).union_.cmdlist, 0);
                    cmd_append_argv(argc, argv, s);
                    free_(s);
                }
            }
        }
    }
}

/// C `vendor/tmux/arguments.c:490`: `struct args_value *args_from_vector(int argc, char **argv)`
pub unsafe fn args_from_vector(argc: i32, argv: *const *mut u8) -> *mut args_value {
    unsafe {
        let values: *mut args_value = xcalloc_(argc as usize).as_ptr();
        for i in 0..argc {
            (*values.add(i as usize)).type_ = args_type::ARGS_STRING;
            (*values.add(i as usize)).union_.string = xstrdup(*argv.add(i as usize)).as_ptr();
        }
        values
    }
}

// TODO change this to use &mut String
macro_rules! args_print_add {
   ($buf:expr, $len:expr, $fmt:literal $(, $args:expr)* $(,)?) => {
        crate::arguments::args_print_add_($buf, $len, format_args!($fmt $(, $args)*))
    };
}
pub unsafe fn args_print_add_(buf: *mut *mut u8, len: *mut usize, fmt: std::fmt::Arguments) {
    unsafe {
        let s = CString::new(fmt.to_string()).unwrap();

        *len += s.as_bytes().len();
        *buf = xrealloc(*buf as *mut c_void, *len).cast().as_ptr();

        strlcat(*buf, s.as_ptr().cast(), *len);
    }
}

/// C `vendor/tmux/arguments.c:524`: `static void args_print_add_value(char **buf, size_t *len, struct args_value *value)`
pub unsafe fn args_print_add_value(buf: *mut *mut u8, len: *mut usize, value: *const args_value) {
    unsafe {
        if **buf != b'\0' {
            args_print_add!(buf, len, " ");
        }

        match (*value).type_ {
            args_type::ARGS_NONE => (),
            args_type::ARGS_COMMANDS => {
                let expanded = cmd_list_print(&*(*value).union_.cmdlist, 0);
                args_print_add!(buf, len, "{{ {} }}", _s(expanded));
                free_(expanded);
            }
            args_type::ARGS_STRING => {
                let expanded = args_escape((*value).union_.string);
                args_print_add!(buf, len, "{}", _s(expanded));
                free_(expanded);
            }
        }
    }
}

/// C `vendor/tmux/arguments.c:548`: `char *args_print(struct args *args)`
pub unsafe fn args_print(args: *mut args) -> *mut u8 {
    unsafe {
        let mut last: *mut args_entry = null_mut();

        let mut len: usize = 1;
        let mut buf: *mut u8 = xcalloc(1, len).cast().as_ptr();

        // Process the flags first.
        for entry in rb_foreach(&raw mut (*args).tree).map(NonNull::as_ptr) {
            if (*entry).flags & ARGS_ENTRY_OPTIONAL_VALUE != 0 {
                continue;
            }
            if !tailq_empty(&raw mut (*entry).values) {
                continue;
            }

            if *buf == b'\0' {
                args_print_add!(&raw mut buf, &raw mut len, "-");
            }
            for _ in 0..(*entry).count {
                args_print_add!(&raw mut buf, &raw mut len, "{}", (*entry).flag as char);
            }
        }

        // Then the flags with arguments.
        for entry in rb_foreach(&raw mut (*args).tree).map(NonNull::as_ptr) {
            if (*entry).flags & ARGS_ENTRY_OPTIONAL_VALUE != 0 {
                if *buf != b'\0' {
                    args_print_add!(&raw mut buf, &raw mut len, " -{}", (*entry).flag as char);
                } else {
                    args_print_add!(&raw mut buf, &raw mut len, "-{}", (*entry).flag as char,);
                }
                last = entry;
                continue;
            }
            if tailq_empty(&raw mut (*entry).values) {
                continue;
            }
            for value in tailq_foreach(&raw mut (*entry).values) {
                {
                    if *buf != b'\0' {
                        args_print_add!(&raw mut buf, &raw mut len, " -{}", (*entry).flag as char,);
                    } else {
                        args_print_add!(&raw mut buf, &raw mut len, "-{}", (*entry).flag as char,);
                    }
                    args_print_add_value(&raw mut buf, &raw mut len, value.as_ptr());
                }
            }
            last = entry;
        }
        if !last.is_null() && ((*last).flags & ARGS_ENTRY_OPTIONAL_VALUE != 0) {
            args_print_add!(&raw mut buf, &raw mut len, " --");
        }

        // And finally the argument vector.
        for i in 0..(*args).count {
            args_print_add_value(&raw mut buf, &raw mut len, (*args).values.add(i as usize));
        }

        buf
    }
}

/// Escape an argument.
/// C `vendor/tmux/arguments.c:606`: `char *args_escape(const char *s)`
pub unsafe fn args_escape(s: *const u8) -> *mut u8 {
    unsafe {
        let dquoted: *const u8 = c!(" #';${}%");
        let squoted: *const u8 = c!(" \"");

        let mut escaped: *mut u8 = null_mut();

        if *s == b'\0' {
            return format_nul!("''");
        }
        let quotes = if *s.add(libc::strcspn(s, dquoted)) != b'\0' {
            Some('"')
        } else if *s.add(libc::strcspn(s, squoted)) != b'\0' {
            Some('\'')
        } else {
            None
        };

        if *s != b' ' && *s.add(1) == b'\0' && (quotes.is_some() || *s == b'~') {
            escaped = format_nul!("\\{}", *s as char);
            return escaped;
        }

        let mut flags =
            vis_flags::VIS_OCTAL | vis_flags::VIS_CSTYLE | vis_flags::VIS_TAB | vis_flags::VIS_NL;
        if quotes == Some('"') {
            flags |= vis_flags::VIS_DQ;
        }
        utf8_stravis(&raw mut escaped, s, flags);

        let result = if quotes == Some('\'') {
            format_nul!("'{}'", _s(escaped))
        } else if quotes == Some('"') {
            if *escaped == b'~' {
                format_nul!("\"\\{}\"", _s(escaped))
            } else {
                format_nul!("\"{}\"", _s(escaped))
            }
        } else if *escaped == b'~' {
            format_nul!("\\{}", _s(escaped))
        } else {
            xstrdup(escaped).as_ptr()
        };
        free_(escaped);

        result
    }
}

// a better name for this might be args_count, but that name already exists
// so it would be confusing to use
pub unsafe fn args_has_count(args: *mut args, flag: u8) -> i32 {
    unsafe {
        let entry = args_find(args, flag);
        if entry.is_null() {
            return 0;
        }
        (*entry).count as i32
    }
}

/// C `vendor/tmux/arguments.c:653`: `int args_has(struct args *args, u_char flag)`
pub unsafe fn args_has(args: *mut args, flag: char) -> bool {
    debug_assert!(flag.is_ascii());

    unsafe {
        let flag = flag as u8;
        let entry = args_find(args, flag);
        if entry.is_null() {
            return false;
        }
        (*entry).count != 0
    }
}

/// C `vendor/tmux/arguments.c:665`: `void args_set(struct args *args, u_char flag, struct args_value *value, int flags)`
pub unsafe fn args_set(args: *mut args, flag: c_uchar, value: *mut args_value, flags: i32) {
    unsafe {
        let mut entry: *mut args_entry = args_find(args, flag);

        if entry.is_null() {
            entry = xcalloc1();
            (*entry).flag = flag;
            (*entry).count = 1;
            (*entry).flags = flags;
            tailq_init(&raw mut (*entry).values);
            rb_insert(&raw mut (*args).tree, entry);
        } else {
            (*entry).count += 1;
        }
        if !value.is_null() && (*value).type_ != args_type::ARGS_NONE {
            tailq_insert_tail(&raw mut (*entry).values, value);
        } else {
            free_(value);
        }
    }
}

/// C `vendor/tmux/arguments.c:687`: `const char *args_get(struct args *args, u_char flag)`
pub unsafe fn args_get(args: *mut args, flag: u8) -> *const u8 {
    unsafe {
        let entry = args_find(args, flag);

        if entry.is_null() {
            return null_mut();
        }
        if tailq_empty(&raw mut (*entry).values) {
            return null_mut();
        }
        (*tailq_last(&raw mut (*entry).values)).union_.string
    }
}

/// C `vendor/tmux/arguments.c:700`: `u_char args_first(struct args *args, struct args_entry **entry)`
pub unsafe fn args_first(args: *mut args, entry: *mut *mut args_entry) -> u8 {
    unsafe {
        *entry = rb_min(&raw mut (*args).tree);
        if (*entry).is_null() {
            return 0;
        }
        (*(*entry)).flag
    }
}

/// Get next argument.
/// C `vendor/tmux/arguments.c:710`: `u_char args_next(struct args_entry **entry)`
pub unsafe fn args_next(entry: *mut *mut args_entry) -> u8 {
    unsafe {
        *entry = rb_next(*entry);
        if (*entry).is_null() {
            return 0;
        }
        (*(*entry)).flag
    }
}

/// Get argument count.
/// C `vendor/tmux/arguments.c:720`: `u_int args_count(struct args *args)`
pub unsafe fn args_count(args: *const args) -> u32 {
    unsafe { (*args).count }
}

/// Get argument values.
/// C `vendor/tmux/arguments.c:727`: `struct args_value *args_values(struct args *args)`
pub unsafe fn args_values(args: *mut args) -> *mut args_value {
    unsafe { (*args).values }
}

/// Get argument value.
/// C `vendor/tmux/arguments.c:734`: `struct args_value *args_value(struct args *args, u_int idx)`
pub unsafe fn args_value(args: *mut args, idx: u32) -> *mut args_value {
    unsafe {
        if idx >= (*args).count {
            return null_mut();
        }
        (*args).values.add(idx as usize)
    }
}

/// Return argument as string.
/// C `vendor/tmux/arguments.c:743`: `const char *args_string(struct args *args, u_int idx)`
pub unsafe fn args_string(args: *mut args, idx: u32) -> *const u8 {
    unsafe {
        if idx >= (*args).count {
            return null();
        }
        args_value_as_string((*args).values.add(idx as usize))
    }
}

/// Make a command now.
/// C `vendor/tmux/arguments.c:752`: `struct cmd_list *args_make_commands_now(struct cmd *self, struct cmdq_item *item, u_int idx, int expand)`
pub unsafe fn args_make_commands_now(
    self_: *mut cmd,
    item: *mut cmdq_item,
    idx: u32,
    expand: bool,
) -> *mut cmd_list {
    unsafe {
        let mut error = null_mut();
        let state = args_make_commands_prepare(self_, item, idx, null_mut(), false, expand);
        let cmdlist = args_make_commands(state, 0, null_mut(), &raw mut error);
        if cmdlist.is_null() {
            cmdq_error!(item, "{}", _s(error));
            free_(error);
        } else {
            (*cmdlist).references += 1;
        }
        args_make_commands_free(state);
        cmdlist
    }
}

/// Save bits to make a command later.
/// C `vendor/tmux/arguments.c:773`: `struct args_command_state *args_make_commands_prepare(struct cmd *self, struct cmdq_item *item, u_int idx, const char *default_command, int wait, int expand)`
pub unsafe fn args_make_commands_prepare<'a>(
    self_: *mut cmd,
    item: *mut cmdq_item,
    idx: u32,
    default_command: *const u8,
    wait: bool,
    expand: bool,
) -> *mut args_command_state<'a> {
    let __func__ = "args_make_commands_prepare";
    unsafe {
        let args = cmd_get_args(self_);
        let target = cmdq_get_target(item);
        let tc = cmdq_get_target_client(item);

        let state = xcalloc1::<args_command_state>() as *mut args_command_state;

        let cmd = if idx < (*args).count {
            let value = (*args).values.add(idx as usize);
            if (*value).type_ == args_type::ARGS_COMMANDS {
                (*state).cmdlist = (*value).union_.cmdlist;
                (*(*state).cmdlist).references += 1;
                return state;
            }
            (*value).union_.string
        } else {
            if default_command.is_null() {
                fatalx("argument out of range");
            }
            default_command
        };

        if expand {
            (*state).cmd = format_single_from_target(item, cmd);
        } else {
            (*state).cmd = xstrdup(cmd).as_ptr();
        }
        log_debug!("{}: {}", __func__, _s((*state).cmd));

        if wait {
            (*state).pi.item = item;
        }
        let mut file = null();
        cmd_get_source(self_, &raw mut file, &(*state).pi.line);
        if !file.is_null() {
            (*state).pi.file = Some(cstr_to_str(xstrdup(file).as_ptr()));
        }
        (*state).pi.c = tc;
        if !(*state).pi.c.is_null() {
            (*(*state).pi.c).references += 1;
        }
        cmd_find_copy_state(&raw mut (*state).pi.fs, target);

        state
    }
}

/// Return argument as command.
/// C `vendor/tmux/arguments.c:822`: `struct cmd_list *args_make_commands(struct args_command_state *state, int argc, char **argv, char **error)`
pub unsafe fn args_make_commands(
    state: *mut args_command_state,
    argc: i32,
    argv: *mut *mut u8,
    error: *mut *mut u8,
) -> *mut cmd_list {
    let __func__ = "args_make_commands";
    unsafe {
        if !(*state).cmdlist.is_null() {
            if argc == 0 {
                return (*state).cmdlist;
            }
            return cmd_list_copy(&*(*state).cmdlist, argc, argv);
        }

        let mut cmd = xstrdup((*state).cmd).as_ptr();
        log_debug!("{}: {}", __func__, _s(cmd));
        cmd_log_argv!(argc, argv, "args_make_commands");
        for i in 0..argc {
            let new_cmd = cmd_template_replace(cmd, cstr_to_str_(*argv.add(i as usize)), i + 1);
            log_debug!(
                "{}: %%{} {}: {}",
                __func__,
                i + 1,
                _s(*argv.add(i as usize)),
                _s(new_cmd)
            );
            free_(cmd);
            cmd = new_cmd;
        }
        log_debug!("{}: {}", __func__, _s(cmd));

        let pr = cmd_parse_from_string(cstr_to_str(cmd), Some(&(*state).pi));
        free_(cmd);

        match pr {
            Err(err) => {
                *error = err;
                null_mut()
            }
            Ok(cmdlist) => cmdlist,
        }
    }
}

#[expect(
    clippy::disallowed_methods,
    reason = "this usage is okay, getting pointer to call free"
)]
/// Free commands state.
/// C `vendor/tmux/arguments.c:860`: `void args_make_commands_free(struct args_command_state *state)`
pub unsafe fn args_make_commands_free(state: *mut args_command_state) {
    unsafe {
        if !(*state).cmdlist.is_null() {
            cmd_list_free((*state).cmdlist);
        }
        if !(*state).pi.c.is_null() {
            server_client_unref((*state).pi.c);
        }
        free_(
            (*state)
                .pi
                .file
                .map(str::as_ptr)
                .unwrap_or_default()
                .cast_mut(),
        ); // TODO casting away const
        free_((*state).cmd);
        free_(state);
    }
}

/// Get prepared command.
/// C `vendor/tmux/arguments.c:873`: `char *args_make_commands_get_command(struct args_command_state *state)`
pub unsafe fn args_make_commands_get_command(state: *mut args_command_state) -> *mut u8 {
    unsafe {
        if !(*state).cmdlist.is_null() {
            let first = cmd_list_first((*state).cmdlist);
            if first.is_null() {
                return xstrdup_(c"").as_ptr();
            }
            return xstrdup__(cmd_get_entry(first).name);
        }
        let n = libc::strcspn((*state).cmd, c!(" ,"));
        format_nul!("{1:0$}", n, _s((*state).cmd))
    }
}

/// Get first value in argument.
/// C `vendor/tmux/arguments.c:892`: `struct args_value *args_first_value(struct args *args, u_char flag)`
pub unsafe fn args_first_value(args: *mut args, flag: u8) -> *mut args_value {
    unsafe {
        let entry = args_find(args, flag);
        if entry.is_null() {
            return null_mut();
        }
        tailq_first(&raw mut (*entry).values)
    }
}

/// Get next value in argument.
/// C `vendor/tmux/arguments.c:903`: `struct args_value *args_next_value(struct args_value *value)`
pub unsafe fn args_next_value(value: *mut args_value) -> *mut args_value {
    unsafe { tailq_next(value) }
}

/// Convert an argument value to a number.
/// C `vendor/tmux/arguments.c:910`: `long long args_strtonum(struct args *args, u_char flag, long long minval, long long maxval, char **cause)`
pub unsafe fn args_strtonum(
    args: *mut args,
    flag: u8,
    minval: i64,
    maxval: i64,
    cause: *mut *mut u8,
) -> i64 {
    unsafe {
        let entry = args_find(args, flag);
        if entry.is_null() {
            *cause = xstrdup_(c"missing").as_ptr();
            return 0;
        }
        let value = tailq_last(&raw mut (*entry).values);
        if value.is_null()
            || (*value).type_ != args_type::ARGS_STRING
            || (*value).union_.string.is_null()
        {
            *cause = xstrdup_(c"missing").as_ptr();
            return 0;
        }

        match strtonum((*value).union_.string, minval, maxval) {
            Ok(ll) => {
                *cause = null_mut();
                ll
            }
            Err(errstr) => {
                *cause = xstrdup(errstr.as_ptr().cast()).as_ptr();
                0
            }
        }
    }
}

/// Convert an argument value to a number, and expand formats.
/// C `vendor/tmux/arguments.c:942`: `long long args_strtonum_and_expand(struct args *args, u_char flag, long long minval, long long maxval, struct cmdq_item *item, char **cause)`
pub unsafe fn args_strtonum_and_expand(
    args: *mut args,
    flag: u8,
    minval: c_longlong,
    maxval: c_longlong,
    item: *mut cmdq_item,
    cause: *mut *mut u8,
) -> c_longlong {
    unsafe {
        let entry = args_find(args, flag);
        if entry.is_null() {
            *cause = xstrdup_(c"missing").as_ptr();
            return 0;
        }
        let value = tailq_last(&raw mut (*entry).values);
        if value.is_null()
            || (*value).type_ != args_type::ARGS_STRING
            || (*value).union_.string.is_null()
        {
            *cause = xstrdup_(c"missing").as_ptr();
            return 0;
        }

        let formatted = format_single_from_target(item, (*value).union_.string);
        let tmp = strtonum(formatted, minval, maxval);
        free_(formatted);
        match tmp {
            Ok(ll) => {
                *cause = null_mut();
                ll
            }
            Err(errstr) => {
                *cause = xstrdup_(errstr).as_ptr();
                0
            }
        }
    }
}

/// Convert an argument to a number which may be a percentage.
/// C `vendor/tmux/arguments.c:977`: `long long args_percentage(struct args *args, u_char flag, long long minval, long long maxval, long long curval, char **cause)`
pub unsafe fn args_percentage(
    args: *mut args,
    flag: u8,
    minval: i64,
    maxval: i64,
    curval: i64,
    cause: *mut *mut u8,
) -> i64 {
    unsafe {
        let entry = args_find(args, flag);
        if entry.is_null() {
            *cause = xstrdup_(c"missing").as_ptr();
            return 0;
        }
        if tailq_empty(&raw mut (*entry).values) {
            *cause = xstrdup_(c"empty").as_ptr();
            return 0;
        }
        let value = (*tailq_last(&raw mut (*entry).values)).union_.string;
        args_string_percentage(value, minval, maxval, curval, cause)
    }
}

/// Convert a string to a number which may be a percentage.
/// C `vendor/tmux/arguments.c:997`: `long long args_string_percentage(const char *value, long long minval, long long maxval, long long curval, char **cause)`
pub unsafe fn args_string_percentage(
    value: *const u8,
    minval: i64,
    maxval: i64,
    curval: i64,
    cause: *mut *mut u8,
) -> i64 {
    unsafe {
        let mut ll: i64;
        let valuelen: usize = strlen(value);
        let copy;

        if valuelen == 0 {
            *cause = xstrdup_(c"empty").as_ptr();
            return 0;
        }
        if *value.add(valuelen - 1) == b'%' {
            copy = xstrdup(value).as_ptr();
            *copy.add(valuelen - 1) = b'\0';

            let tmp = strtonum(copy, 0, 100);
            free_(copy);
            ll = match tmp {
                Ok(n) => n,
                Err(errstr) => {
                    *cause = xstrdup_(errstr).as_ptr();
                    return 0;
                }
            };
            ll = (curval * ll) / 100;
            if ll < minval {
                *cause = xstrdup_(c"too small").as_ptr();
                return 0;
            }
            if ll > maxval {
                *cause = xstrdup_(c"too large").as_ptr();
                return 0;
            }
        } else {
            ll = match strtonum(value, minval, maxval) {
                Ok(n) => n,
                Err(errstr) => {
                    *cause = xstrdup_(errstr).as_ptr();
                    return 0;
                }
            };
        }

        *cause = null_mut();
        ll
    }
}

/// Convert an argument to a number which may be a percentage, and expand formats.
/// C `vendor/tmux/arguments.c:1045`: `long long args_percentage_and_expand(struct args *args, u_char flag, long long minval, long long maxval, long long curval, struct cmdq_item *item, char **cause)`
pub unsafe fn args_percentage_and_expand(
    args: *mut args,
    flag: u8,
    minval: i64,
    maxval: i64,
    curval: i64,
    item: *mut cmdq_item,
    cause: *mut *mut u8,
) -> i64 {
    unsafe {
        let entry = args_find(args, flag);
        if entry.is_null() {
            *cause = xstrdup_(c"missing").as_ptr();
            return 0;
        }
        if tailq_empty(&raw mut (*entry).values) {
            *cause = xstrdup_(c"empty").as_ptr();
            return 0;
        }
        let value = (*tailq_last(&raw mut (*entry).values)).union_.string;
        args_string_percentage_and_expand(value, minval, maxval, curval, item, cause)
    }
}

/// Convert a string to a number which may be a percentage, and expand formats.
/// C `vendor/tmux/arguments.c:1068`: `long long args_string_percentage_and_expand(const char *value, long long minval, long long maxval, long long curval, struct cmdq_item *item, char **cause)`
pub unsafe fn args_string_percentage_and_expand(
    value: *const u8,
    minval: i64,
    maxval: i64,
    curval: i64,
    item: *mut cmdq_item,
    cause: *mut *mut u8,
) -> i64 {
    unsafe {
        let valuelen = strlen(value);
        let mut ll: i64;
        let f: *mut u8;

        if *value.add(valuelen - 1) == b'%' {
            let copy = xstrdup(value).as_ptr();
            *copy.add(valuelen - 1) = b'\0';

            f = format_single_from_target(item, copy);
            let tmp = strtonum(f, 0, 100);
            free_(f);
            free_(copy);
            ll = match tmp {
                Ok(n) => n,
                Err(errstr) => {
                    *cause = xstrdup_(errstr).as_ptr();
                    return 0;
                }
            };
            ll = (curval * ll) / 100;
            if ll < minval {
                *cause = xstrdup_(c"too small").as_ptr();
                return 0;
            }
            if ll > maxval {
                *cause = xstrdup_(c"too large").as_ptr();
                return 0;
            }
        } else {
            f = format_single_from_target(item, value);
            let tmp = strtonum(f, minval, maxval);
            free_(f);
            ll = match tmp {
                Ok(n) => n,
                Err(errstr) => {
                    *cause = xstrdup_(errstr).as_ptr();
                    return 0;
                }
            };
        }

        *cause = null_mut();
        ll
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Read a NUL-terminated C string into an owned byte vec (no NUL).
    unsafe fn read_cstr(p: *const u8) -> Vec<u8> {
        let n = unsafe { crate::libc::strlen(p) };
        unsafe { std::slice::from_raw_parts(p, n) }.to_vec()
    }

    // Run `bytes` (NUL-terminated internally) through args_escape and return the
    // escaped result as an owned byte vec, freeing the heap buffer args_escape
    // returns.
    unsafe fn escape(bytes: &[u8]) -> Vec<u8> {
        let mut c = bytes.to_vec();
        c.push(0);
        unsafe {
            let out = args_escape(c.as_ptr());
            let v = read_cstr(out);
            free_(out);
            v
        }
    }

    // Build an ARGS_STRING value owning a fresh copy of `s`.
    unsafe fn strvalue(s: *const u8) -> args_value {
        unsafe {
            let mut v: args_value = zeroed();
            v.type_ = args_type::ARGS_STRING;
            v.union_.string = xstrdup(s).as_ptr();
            v
        }
    }

    #[test]
    fn args_type_to_string_variants() {
        assert_eq!(args_type_to_string(args_type::ARGS_NONE), "NONE");
        assert_eq!(args_type_to_string(args_type::ARGS_STRING), "STRING");
        assert_eq!(args_type_to_string(args_type::ARGS_COMMANDS), "COMMANDS");
    }

    #[test]
    fn args_escape_no_quoting() {
        // Plain printable string with no special characters is returned as-is.
        unsafe {
            assert_eq!(escape(b"hello"), b"hello");
        }
    }

    #[test]
    fn args_escape_empty_string() {
        // Empty string becomes two single quotes.
        unsafe {
            assert_eq!(escape(b""), b"''");
        }
    }

    #[test]
    fn args_escape_double_quotes() {
        // A space, '#', or '$' is in the double-quote set, so the whole value is
        // wrapped in double quotes.
        unsafe {
            assert_eq!(escape(b"a b"), b"\"a b\"");
            assert_eq!(escape(b"a#b"), b"\"a#b\"");
            // Inside double quotes, VIS_DQ backslash-escapes a `$` that precedes a
            // name char so it can't be re-expanded as a variable on re-parse.
            assert_eq!(escape(b"a$b"), b"\"a\\$b\"");
        }
    }

    #[test]
    fn args_escape_single_quotes() {
        // A bare double quote (with no double-quote-set char present) forces
        // single quoting instead.
        unsafe {
            assert_eq!(escape(b"a\"b"), b"'a\"b'");
        }
    }

    #[test]
    fn args_escape_tilde() {
        // A lone '~' is backslash-escaped; a leading '~' followed by more text is
        // likewise backslash-escaped at the front.
        unsafe {
            assert_eq!(escape(b"~"), b"\\~");
            assert_eq!(escape(b"~abc"), b"\\~abc");
        }
    }

    #[test]
    fn args_escape_control_and_multibyte_octal() {
        // A control byte and a lone (invalid-UTF-8) continuation byte are both
        // octal-escaped by vis(3).
        unsafe {
            assert_eq!(escape(&[0x01]), b"\\001");
            assert_eq!(escape(&[0x82]), b"\\202");
        }
    }

    #[test]
    fn args_parse_flag_and_positional() {
        // argv = ["cmd", "-a", "hello"] with template "a" (boolean flag) parses
        // -a as a flag and "hello" as the single positional argument.
        unsafe {
            let mut values = [
                strvalue(crate::c!("cmd")),
                strvalue(crate::c!("-a")),
                strvalue(crate::c!("hello")),
            ];
            let parse = args_parse::new("a", -1, -1, None);
            let mut cause: *mut u8 = null_mut();

            let args = args_parse(&parse, values.as_mut_ptr(), values.len() as u32, &raw mut cause);

            assert!(!args.is_null());
            assert!(cause.is_null());
            assert!(args_has(args, 'a'));
            assert_eq!(args_count(args), 1);
            assert_eq!(read_cstr(args_string(args, 0)), b"hello");

            args_free(args);
            args_free_values(values.as_mut_ptr(), values.len() as u32);
        }
    }

    // Build a vector of ARGS_STRING values from &str slices (each xstrdup'd inside
    // strvalue, so the temporary CStrings can drop immediately).
    unsafe fn build_values(argv: &[&str]) -> Vec<args_value> {
        argv.iter()
            .map(|s| {
                let c = CString::new(*s).unwrap();
                unsafe { strvalue(c.as_ptr().cast()) }
            })
            .collect()
    }

    // Run args_parse over `argv` (argv[0] is the command word, skipped like C).
    // Frees the source values before returning; caller owns the returned args
    // (if non-null) and cause (if non-null).
    unsafe fn run(parse: &args_parse, argv: &[&str]) -> (*mut args, *mut u8) {
        unsafe {
            let mut values = build_values(argv);
            let mut cause: *mut u8 = null_mut();
            let args = args_parse(parse, values.as_mut_ptr(), values.len() as u32, &raw mut cause);
            args_free_values(values.as_mut_ptr(), values.len() as u32);
            (args, cause)
        }
    }

    // arguments.c:147 args_parse_flag_argument: a required-argument flag (template
    // "a:") given inline ("-avalue") captures the trailing text as the value.
    #[test]
    fn parse_flag_value_attached() {
        unsafe {
            let parse = args_parse::new("a:", -1, -1, None);
            let (args, cause) = run(&parse, &["cmd", "-avalue"]);
            assert!(!args.is_null());
            assert!(cause.is_null());
            assert_eq!(read_cstr(args_get(args, b'a')), b"value");
            args_free(args);
        }
    }

    // arguments.c:147: a required-argument flag ("a:") consumes the following
    // separate argument as its value.
    #[test]
    fn parse_flag_value_separate() {
        unsafe {
            let parse = args_parse::new("a:", -1, -1, None);
            let (args, cause) = run(&parse, &["cmd", "-a", "value"]);
            assert!(!args.is_null());
            assert!(cause.is_null());
            assert_eq!(read_cstr(args_get(args, b'a')), b"value");
            // The value was consumed as the flag argument, not left positional.
            assert_eq!(args_count(args), 0);
            args_free(args);
        }
    }

    // arguments.c:180: a required-argument flag at end of input with no value is
    // an error ("-a expects an argument").
    #[test]
    fn parse_flag_missing_required_argument() {
        unsafe {
            let parse = args_parse::new("a:", -1, -1, None);
            let (args, cause) = run(&parse, &["cmd", "-a"]);
            assert!(args.is_null());
            assert_eq!(read_cstr(cause), b"-a expects an argument");
            free_(cause);
        }
    }

    // arguments.c:173 (`found[2] == ':'`): an optional-argument flag ("a::") at
    // end of input is set with no value (ARGS_ENTRY_OPTIONAL_VALUE); args_has is
    // true but args_get is NULL.
    #[test]
    fn parse_optional_flag_no_value() {
        unsafe {
            let parse = args_parse::new("a::", -1, -1, None);
            let (args, cause) = run(&parse, &["cmd", "-a"]);
            assert!(!args.is_null());
            assert!(cause.is_null());
            assert!(args_has(args, 'a'));
            assert!(args_get(args, b'a').is_null());
            args_free(args);
        }
    }

    // arguments.c:184-192: for an optional-argument flag, if the next value looks
    // like a flag (starts '-' then '-' or a letter), the flag is treated as
    // valueless (ARGS_ENTRY_OPTIONAL_VALUE) and the "-b" is left to be parsed as
    // its own flag. So "-a" does NOT swallow "-b"; -a has no value and -b is set.
    #[test]
    fn parse_optional_flag_leaves_following_flag() {
        unsafe {
            let parse = args_parse::new("a::b", -1, -1, None);
            let (args, cause) = run(&parse, &["cmd", "-a", "-b"]);
            assert!(!args.is_null());
            assert!(cause.is_null());
            // -a did not swallow "-b"; it has no value.
            assert!(args_has(args, 'a'));
            assert!(args_get(args, b'a').is_null());
            // -b is recorded as its own flag.
            assert!(args_has(args, 'b'));
            args_free(args);
        }
    }

    // arguments.c:184-194: an optional-argument flag DOES take a following token
    // as its value when that token does not look like a flag (does not start with
    // '-' followed by '-' or a letter). Here "-a val" sets -a to "val".
    #[test]
    fn parse_optional_flag_takes_nonflag_value() {
        unsafe {
            let parse = args_parse::new("a::b", -1, -1, None);
            let (args, cause) = run(&parse, &["cmd", "-a", "val"]);
            assert!(!args.is_null());
            assert!(cause.is_null());
            assert_eq!(read_cstr(args_get(args, b'a')), b"val");
            assert!(!args_has(args, 'b'));
            args_free(args);
        }
    }

    // arguments.c:207 args_parse_flags: combined boolean flags in one token
    // ("-ab") set each flag.
    #[test]
    fn parse_combined_boolean_flags() {
        unsafe {
            let parse = args_parse::new("ab", -1, -1, None);
            let (args, cause) = run(&parse, &["cmd", "-ab"]);
            assert!(!args.is_null());
            assert!(cause.is_null());
            assert!(args_has(args, 'a'));
            assert!(args_has(args, 'b'));
            args_free(args);
        }
    }

    // arguments.c:238: a flag not present in the template yields "unknown flag".
    #[test]
    fn parse_unknown_flag() {
        unsafe {
            let parse = args_parse::new("a", -1, -1, None);
            let (args, cause) = run(&parse, &["cmd", "-z"]);
            assert!(args.is_null());
            assert_eq!(read_cstr(cause), b"unknown flag -z");
            free_(cause);
        }
    }

    // arguments.c:233: a non-alphanumeric flag character is rejected as "invalid
    // flag" (before the template is even consulted).
    #[test]
    fn parse_invalid_flag_char() {
        unsafe {
            let parse = args_parse::new("a", -1, -1, None);
            let (args, cause) = run(&parse, &["cmd", "-@"]);
            assert!(args.is_null());
            assert_eq!(read_cstr(cause), b"invalid flag -@");
            free_(cause);
        }
    }

    // arguments.c:206 (`string[0] == '-' && string[1] == '\0'`): a lone "--" stops
    // flag parsing; everything after is positional (so "-a" becomes an argument,
    // not a flag).
    #[test]
    fn parse_double_dash_terminates_flags() {
        unsafe {
            let parse = args_parse::new("a", -1, -1, None);
            let (args, cause) = run(&parse, &["cmd", "--", "-a"]);
            assert!(!args.is_null());
            assert!(cause.is_null());
            assert!(!args_has(args, 'a'));
            assert_eq!(args_count(args), 1);
            assert_eq!(read_cstr(args_string(args, 0)), b"-a");
            args_free(args);
        }
    }

    // arguments.c:665 args_set increments the count when the same flag repeats;
    // args_has_count reports it.
    #[test]
    fn parse_repeated_boolean_flag_counts() {
        unsafe {
            let parse = args_parse::new("a", -1, -1, None);
            let (args, cause) = run(&parse, &["cmd", "-a", "-a"]);
            assert!(!args.is_null());
            assert!(cause.is_null());
            assert_eq!(args_has_count(args, b'a'), 2);
            args_free(args);
        }
    }

    // arguments.c:687 args_get returns the LAST value (tailq_last) when a
    // value-flag is repeated; the count reflects both occurrences.
    #[test]
    fn parse_repeated_value_flag_returns_last() {
        unsafe {
            let parse = args_parse::new("a:", -1, -1, None);
            let (args, cause) = run(&parse, &["cmd", "-a", "one", "-a", "two"]);
            assert!(!args.is_null());
            assert!(cause.is_null());
            assert_eq!(read_cstr(args_get(args, b'a')), b"two");
            assert_eq!(args_has_count(args, b'a'), 2);
            args_free(args);
        }
    }

    // arguments.c:340 (`parse->lower`): fewer positional args than the lower bound
    // is an error naming the minimum.
    #[test]
    fn parse_too_few_arguments() {
        unsafe {
            let parse = args_parse::new("", 2, -1, None);
            let (args, cause) = run(&parse, &["cmd", "only-one"]);
            assert!(args.is_null());
            assert_eq!(read_cstr(cause), b"too few arguments (need at least 2)");
            free_(cause);
        }
    }

    // arguments.c:345 (`parse->upper`): more positional args than the upper bound
    // is an error naming the maximum.
    #[test]
    fn parse_too_many_arguments() {
        unsafe {
            let parse = args_parse::new("", -1, 1, None);
            let (args, cause) = run(&parse, &["cmd", "a", "b"]);
            assert!(args.is_null());
            assert_eq!(read_cstr(cause), b"too many arguments (need at most 1)");
            free_(cause);
        }
    }

    // arguments.c:260: count == 0 short-circuits to an empty args set.
    #[test]
    fn parse_zero_count_empty() {
        unsafe {
            let parse = args_parse::new("a", -1, -1, None);
            let mut cause: *mut u8 = null_mut();
            let args = args_parse(&parse, null_mut(), 0, &raw mut cause);
            assert!(!args.is_null());
            assert!(cause.is_null());
            assert_eq!(args_count(args), 0);
            args_free(args);
        }
    }

    // arguments.c:700/710 args_first/args_next iterate flags in sorted (args_cmp)
    // order regardless of the order they were parsed.
    #[test]
    fn args_first_next_sorted_order() {
        unsafe {
            let parse = args_parse::new("abc", -1, -1, None);
            let (args, _cause) = run(&parse, &["cmd", "-c", "-a", "-b"]);
            assert!(!args.is_null());
            let mut entry: *mut args_entry = null_mut();
            assert_eq!(args_first(args, &raw mut entry), b'a');
            assert_eq!(args_next(&raw mut entry), b'b');
            assert_eq!(args_next(&raw mut entry), b'c');
            // Exhausted: returns 0 and NULLs the entry.
            assert_eq!(args_next(&raw mut entry), 0);
            assert!(entry.is_null());
            args_free(args);
        }
    }

    // arguments.c:734/743 args_value/args_string return NULL for an out-of-range
    // index, and the stored value otherwise.
    #[test]
    fn args_value_and_string_bounds() {
        unsafe {
            let parse = args_parse::new("", -1, -1, None);
            let (args, _cause) = run(&parse, &["cmd", "pos"]);
            assert!(!args.is_null());
            assert_eq!(args_count(args), 1);
            assert!(!args_value(args, 0).is_null());
            assert!(args_value(args, 1).is_null());
            assert_eq!(read_cstr(args_string(args, 0)), b"pos");
            assert!(args_string(args, 5).is_null());
            args_free(args);
        }
    }

    // arguments.c:910 args_strtonum: a valid numeric flag value parses within
    // range and clears cause.
    #[test]
    fn strtonum_valid() {
        unsafe {
            let parse = args_parse::new("n:", -1, -1, None);
            let (args, _cause) = run(&parse, &["cmd", "-n", "42"]);
            let mut cause: *mut u8 = null_mut();
            let n = args_strtonum(args, b'n', 0, 100, &raw mut cause);
            assert_eq!(n, 42);
            assert!(cause.is_null());
            args_free(args);
        }
    }

    // arguments.c:915 args_strtonum: a missing flag yields 0 and cause "missing".
    #[test]
    fn strtonum_missing_flag() {
        unsafe {
            let parse = args_parse::new("n:", -1, -1, None);
            let (args, _cause) = run(&parse, &["cmd"]);
            let mut cause: *mut u8 = null_mut();
            let n = args_strtonum(args, b'n', 0, 100, &raw mut cause);
            assert_eq!(n, 0);
            assert_eq!(read_cstr(cause), b"missing");
            free_(cause);
            args_free(args);
        }
    }

    // arguments.c:930 args_strtonum: an out-of-range value returns 0 and passes
    // the strtonum error string ("too large") through cause.
    #[test]
    fn strtonum_out_of_range() {
        unsafe {
            let parse = args_parse::new("n:", -1, -1, None);
            let (args, _cause) = run(&parse, &["cmd", "-n", "999"]);
            let mut cause: *mut u8 = null_mut();
            let n = args_strtonum(args, b'n', 0, 100, &raw mut cause);
            assert_eq!(n, 0);
            assert_eq!(read_cstr(cause), b"too large");
            free_(cause);
            args_free(args);
        }
    }

    // arguments.c:997 args_string_percentage: a plain number parses directly
    // within [minval, maxval].
    #[test]
    fn string_percentage_plain_number() {
        unsafe {
            let mut cause: *mut u8 = null_mut();
            let n = args_string_percentage(c!("42"), 0, 100, 0, &raw mut cause);
            assert_eq!(n, 42);
            assert!(cause.is_null());
        }
    }

    // arguments.c:997: a trailing '%' scales curval: 50% of 200 == 100.
    #[test]
    fn string_percentage_of_curval() {
        unsafe {
            let mut cause: *mut u8 = null_mut();
            let n = args_string_percentage(c!("50%"), 0, 1000, 200, &raw mut cause);
            assert_eq!(n, 100);
            assert!(cause.is_null());
        }
    }

    // arguments.c:1013: a scaled percentage below minval reports "too small".
    #[test]
    fn string_percentage_too_small() {
        unsafe {
            let mut cause: *mut u8 = null_mut();
            // 50% of 200 == 100, below minval 200.
            let n = args_string_percentage(c!("50%"), 200, 1000, 200, &raw mut cause);
            assert_eq!(n, 0);
            assert_eq!(read_cstr(cause), b"too small");
            free_(cause);
        }
    }

    // arguments.c:1005: a percentage numerator outside 0..100 fails via strtonum
    // ("too large").
    #[test]
    fn string_percentage_bad_numerator() {
        unsafe {
            let mut cause: *mut u8 = null_mut();
            let n = args_string_percentage(c!("150%"), 0, 1000, 200, &raw mut cause);
            assert_eq!(n, 0);
            assert_eq!(read_cstr(cause), b"too large");
            free_(cause);
        }
    }

    // arguments.c:1002: an empty string is rejected as "empty".
    #[test]
    fn string_percentage_empty() {
        unsafe {
            let mut cause: *mut u8 = null_mut();
            let n = args_string_percentage(c!(""), 0, 100, 0, &raw mut cause);
            assert_eq!(n, 0);
            assert_eq!(read_cstr(cause), b"empty");
            free_(cause);
        }
    }

    // arguments.c:490 args_from_vector builds one ARGS_STRING value per argv
    // element, deep-copying each string.
    #[test]
    fn from_vector_makes_string_values() {
        unsafe {
            let argv: [*mut u8; 2] = [
                xstrdup(c!("hello")).as_ptr(),
                xstrdup(c!("world")).as_ptr(),
            ];
            let values = args_from_vector(2, argv.as_ptr());
            assert!((*values.add(0)).type_ == args_type::ARGS_STRING);
            assert_eq!(read_cstr((*values.add(0)).union_.string), b"hello");
            assert_eq!(read_cstr((*values.add(1)).union_.string), b"world");
            args_free_values(values, 2);
            free_(values);
            free_(argv[0]);
            free_(argv[1]);
        }
    }

    // arguments.c:85 args_copy_value deep-copies an ARGS_STRING (distinct pointer,
    // equal contents).
    #[test]
    fn copy_value_string_is_deep() {
        unsafe {
            let src = strvalue(c!("payload"));
            let mut dst: args_value = zeroed();
            args_copy_value(&raw mut dst, &raw const src);
            assert!(dst.type_ == args_type::ARGS_STRING);
            assert_eq!(read_cstr(dst.union_.string), b"payload");
            assert_ne!(dst.union_.string, src.union_.string);
            args_free_value(&raw mut dst);
            let mut s = src;
            args_free_value(&raw mut s);
        }
    }

    // arguments.c:119 args_value_as_string: NONE renders as the empty string,
    // STRING renders as its bytes.
    #[test]
    fn value_as_string_variants() {
        unsafe {
            let none: args_value = zeroed(); // type_ defaults to ARGS_NONE (0)
            assert_eq!(read_cstr(args_value_as_string(&none as *const _ as *mut _)), b"");
            let mut sv = strvalue(c!("txt"));
            assert_eq!(read_cstr(args_value_as_string(&raw mut sv)), b"txt");
            args_free_value(&raw mut sv);
        }
    }

    // arguments.c:606 args_escape: single characters in the double-quote set that
    // sit alone are backslash-escaped (the len==1 fast path), not quote-wrapped.
    #[test]
    fn escape_single_special_chars() {
        unsafe {
            assert_eq!(escape(b";"), b"\\;");
            assert_eq!(escape(b"%"), b"\\%");
            assert_eq!(escape(b"$"), b"\\$");
        }
    }

    // arguments.c:606 args_escape: a multi-char value containing a double-quote-set
    // char is wrapped in double quotes.
    #[test]
    fn escape_multichar_double_quoted() {
        unsafe {
            assert_eq!(escape(b"a;b"), b"\"a;b\"");
            assert_eq!(escape(b"{}"), b"\"{}\"");
        }
    }

    // arguments.c:653 args_has / arguments.c:687 args_get: an absent flag reports
    // false and NULL, distinct from a present-but-valueless flag.
    #[test]
    fn has_and_get_absent_flag() {
        unsafe {
            let parse = args_parse::new("a", -1, -1, None);
            let (args, cause) = run(&parse, &["cmd"]);
            assert!(!args.is_null());
            assert!(cause.is_null());
            assert!(!args_has(args, 'a'));
            assert!(args_get(args, b'a').is_null());
            assert_eq!(args_has_count(args, b'a'), 0);
            args_free(args);
        }
    }

    // arguments.c:1017 (`ll > maxval`): a scaled percentage above maxval reports
    // "too large". 100% of 200 == 200, above maxval 100.
    #[test]
    fn string_percentage_too_large() {
        unsafe {
            let mut cause: *mut u8 = null_mut();
            let n = args_string_percentage(c!("100%"), 0, 100, 200, &raw mut cause);
            assert_eq!(n, 0);
            assert_eq!(read_cstr(cause), b"too large");
            free_(cause);
        }
    }

    // arguments.c:997 args_string_percentage: 0% scales to 0 (curval * 0 / 100),
    // which is in range and clears cause (distinct from the "empty" rejection).
    #[test]
    fn string_percentage_zero() {
        unsafe {
            let mut cause: *mut u8 = null_mut();
            let n = args_string_percentage(c!("0%"), 0, 100, 200, &raw mut cause);
            assert_eq!(n, 0);
            assert!(cause.is_null());
        }
    }

    // arguments.c:910 args_strtonum: a non-numeric value fails via strtonum and
    // passes its "invalid" error string through cause.
    #[test]
    fn strtonum_invalid_value() {
        unsafe {
            let parse = args_parse::new("n:", -1, -1, None);
            let (args, _cause) = run(&parse, &["cmd", "-n", "abc"]);
            let mut cause: *mut u8 = null_mut();
            let n = args_strtonum(args, b'n', 0, 100, &raw mut cause);
            assert_eq!(n, 0);
            assert_eq!(read_cstr(cause), b"invalid");
            free_(cause);
            args_free(args);
        }
    }

    // arguments.c:892 args_first_value / arguments.c:903 args_next_value walk the
    // per-flag value list in insertion order for a repeated value-flag.
    #[test]
    fn first_next_value_iterates() {
        unsafe {
            let parse = args_parse::new("a:", -1, -1, None);
            let (args, _cause) = run(&parse, &["cmd", "-a", "one", "-a", "two"]);
            assert!(!args.is_null());
            let first = args_first_value(args, b'a');
            assert!(!first.is_null());
            assert_eq!(read_cstr((*first).union_.string), b"one");
            let next = args_next_value(first);
            assert!(!next.is_null());
            assert_eq!(read_cstr((*next).union_.string), b"two");
            // No third value.
            assert!(args_next_value(next).is_null());
            args_free(args);
        }
    }

    // arguments.c:464 args_to_vector emits one string per positional argument (the
    // args->values vector), in order. Flags are not included.
    #[test]
    fn to_vector_positional_args() {
        unsafe {
            let parse = args_parse::new("a", -1, -1, None);
            let (args, _cause) = run(&parse, &["cmd", "-a", "x", "y"]);
            assert!(!args.is_null());
            let mut argc: i32 = 0;
            let mut argv: *mut *mut u8 = null_mut();
            args_to_vector(args, &raw mut argc, &raw mut argv);
            assert_eq!(argc, 2);
            assert_eq!(read_cstr(*argv.add(0)), b"x");
            assert_eq!(read_cstr(*argv.add(1)), b"y");
            cmd_free_argv(argc, argv);
            args_free(args);
        }
    }

    // arguments.c:207 args_parse_flags: a boolean flag combined in one token with a
    // required-argument flag consumes the trailing text as the latter's value:
    // "-abVALUE" with template "ab:" sets -a and -b == "VALUE".
    #[test]
    fn parse_combined_flag_with_value() {
        unsafe {
            let parse = args_parse::new("ab:", -1, -1, None);
            let (args, cause) = run(&parse, &["cmd", "-abVALUE"]);
            assert!(!args.is_null());
            assert!(cause.is_null());
            assert!(args_has(args, 'a'));
            assert_eq!(read_cstr(args_get(args, b'b')), b"VALUE");
            args_free(args);
        }
    }

    // arguments.c:180: a required-argument flag combined at the end of a token with
    // no trailing text and no following argument is an error ("-b expects an
    // argument"). "-ab" with template "ab:" sets -a then fails on -b.
    #[test]
    fn parse_combined_flag_missing_value() {
        unsafe {
            let parse = args_parse::new("ab:", -1, -1, None);
            let (args, cause) = run(&parse, &["cmd", "-ab"]);
            assert!(args.is_null());
            assert_eq!(read_cstr(cause), b"-b expects an argument");
            free_(cause);
        }
    }

    // arguments.c:169-180: a *required*-argument flag (template "a:", not "a::")
    // does NOT apply the looks-like-a-flag heuristic, so "-a -b" consumes "-b" as
    // the literal value of -a (contrast parse_optional_flag_leaves_following_flag).
    #[test]
    fn parse_required_flag_consumes_flaglike_value() {
        unsafe {
            let parse = args_parse::new("a:b", -1, -1, None);
            let (args, cause) = run(&parse, &["cmd", "-a", "-b"]);
            assert!(!args.is_null());
            assert!(cause.is_null());
            assert_eq!(read_cstr(args_get(args, b'a')), b"-b");
            // "-b" was swallowed as a value, not parsed as its own flag.
            assert!(!args_has(args, 'b'));
            args_free(args);
        }
    }

    // arguments.c:85 args_copy_value: copying an ARGS_NONE value leaves the
    // destination as ARGS_NONE with no string allocated.
    #[test]
    fn copy_value_none_stays_none() {
        unsafe {
            let src: args_value = zeroed(); // ARGS_NONE (0)
            let mut dst: args_value = zeroed();
            dst.type_ = args_type::ARGS_STRING; // ensure copy actually overwrites
            dst.union_.string = null_mut();
            args_copy_value(&raw mut dst, &raw const src);
            assert!(dst.type_ == args_type::ARGS_NONE);
        }
    }

    // arguments.c:619 args_escape: a lone space is not a candidate for the len==1
    // backslash fast path (which excludes ' '), so it is double-quoted instead.
    #[test]
    fn escape_single_space() {
        unsafe {
            assert_eq!(escape(b" "), b"\" \"");
        }
    }
}
