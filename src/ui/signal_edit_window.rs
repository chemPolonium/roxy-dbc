#[allow(dead_code)]
#[derive(Clone)]
pub struct SignalEditWindowState {
    // 这里并不需要 is_open 字段
    // 因为 Signal Edit Window 必然依附于 Message Edit Window 存在
    // 关闭窗口的时候这个状态直接就被删除了
    pub signal_name: String,
}

#[allow(dead_code)]
/// Signal 编辑对话框状态
pub struct SignalEditDialog {
    pub show: bool,
    pub parent_dbc_id: usize,
    pub message_id: u32,

    // 编辑缓冲区
    pub name_buffer: String,
    pub start_bit_buffer: String,
    pub size_buffer: String,
    pub byte_order_is_little: bool,
    pub signed: bool,
    pub factor_buffer: String,
    pub offset_buffer: String,
    pub min_buffer: String,
    pub max_buffer: String,
    pub unit_buffer: String,
    pub comment_buffer: String,

    // 原始值（用于取消）
    pub original_name: String,
}

#[allow(dead_code)]
impl SignalEditDialog {
    pub fn new() -> Self {
        Self {
            show: false,
            parent_dbc_id: 0,
            message_id: 0,
            name_buffer: String::new(),
            start_bit_buffer: String::new(),
            size_buffer: String::new(),
            byte_order_is_little: true,
            signed: false,
            factor_buffer: String::from("1.0"),
            offset_buffer: String::from("0.0"),
            min_buffer: String::from("0.0"),
            max_buffer: String::from("0.0"),
            unit_buffer: String::new(),
            comment_buffer: String::new(),
            original_name: String::new(),
        }
    }

    pub fn open(&mut self, parent_dbc_id: usize, message_id: u32) {
        self.show = true;
        self.parent_dbc_id = parent_dbc_id;
        self.message_id = message_id;
        // other fields should be initialized by caller using actual signal data
    }

    pub fn close(&mut self) {
        self.show = false;
    }
}

impl Default for SignalEditDialog {
    fn default() -> Self {
        Self::new()
    }
}

