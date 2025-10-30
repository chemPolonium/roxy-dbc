#[derive(Clone)]
pub struct SignalEditWindowState {
    // 这里并不需要 is_open 字段
    // 因为 Signal Edit Window 必然依附于 Message Edit Window 存在
    // 关闭窗口的时候这个状态直接就被删除了
    pub signal_name: String,
}
