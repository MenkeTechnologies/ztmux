/// C `vendor/tmux/compat/getdtablecount.c:32`: `int getdtablecount(void)`
// Faithful compat shim. The new vendored imsg read path (msgbuf_read/ibuf_read)
// no longer does the fd-table headroom check the old imsg_read did, so this is
// currently unused but kept to mirror the compat layer.
#[expect(dead_code)]
pub fn getdtablecount() -> i32 {
    if let Ok(read_dir) = std::fs::read_dir("/proc/self/fd") {
        read_dir.count() as i32
    } else {
        0
    }
}
