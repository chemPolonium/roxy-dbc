/// Message 新建对话框状态
pub struct MessageCreateDialog {
    pub show: bool,
    pub parent_dbc_id: usize,

    // 输入缓冲区
    pub name_buffer: String,
    pub comment_buffer: String,
    pub id_buffer: String,
    pub size_buffer: String,
    pub transmitter_buffer: String,
}

impl MessageCreateDialog {
    pub fn new() -> Self {
        Self {
            show: false,
            parent_dbc_id: 0,
            name_buffer: String::new(),
            comment_buffer: String::new(),
            id_buffer: String::new(),
            size_buffer: String::from("8"), // 默认8字节
            transmitter_buffer: String::new(),
        }
    }

    /// 打开创建对话框
    pub fn open(&mut self, parent_dbc_id: usize, suggested_id: u32) {
        self.show = true;
        self.parent_dbc_id = parent_dbc_id;

        // 重置所有字段
        self.name_buffer.clear();
        self.comment_buffer.clear();
        self.id_buffer = format!("0x{:X}", suggested_id);
        self.size_buffer = String::from("8");
        self.transmitter_buffer.clear();
    }

    /// 关闭对话框
    pub fn close(&mut self) {
        self.show = false;
    }

    /// 检查输入是否有效
    pub fn is_valid(&self) -> bool {
        !self.name_buffer.trim().is_empty()
            && !self.id_buffer.trim().is_empty()
            && self.parse_id().is_some()
            && self.parse_size().is_some()
    }

    /// 解析 ID
    pub fn parse_id(&self) -> Option<u32> {
        let s = self.id_buffer.trim();
        if s.is_empty() {
            return None;
        }
        // 尝试解析十六进制（0x 或 0X 前缀）
        if s.starts_with("0x") || s.starts_with("0X") {
            if let Ok(id) = u32::from_str_radix(&s[2..], 16) {
                return Some(id);
            }
        }
        // 尝试解析十进制
        if let Ok(id) = s.parse::<u32>() {
            return Some(id);
        }
        // 尝试直接解析为十六进制（没有 0x 前缀）
        if let Ok(id) = u32::from_str_radix(s, 16) {
            return Some(id);
        }
        None
    }

    /// 解析 Size
    pub fn parse_size(&self) -> Option<u64> {
        let s = self.size_buffer.trim();
        if let Ok(size) = s.parse::<u64>() {
            if size <= 8 {
                return Some(size);
            }
        }
        None
    }
}

impl Default for MessageCreateDialog {
    fn default() -> Self {
        Self::new()
    }
}
