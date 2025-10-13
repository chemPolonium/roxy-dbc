//! DBC 文件读取和解析模块
use std::fs;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct DbcData {
    pub file_path: String,
    pub messages: Vec<MessageInfo>,
    pub error_message: String,
}

#[derive(Debug, Clone)]
pub struct MessageInfo {
    pub id: u32,
    pub name: String,
    pub length: u8,
    pub signals: Vec<SignalInfo>,
}

#[derive(Debug, Clone)]
pub struct SignalInfo {
    pub name: String,
    pub start_bit: u8,
    pub length: u8,
    pub factor: f64,
    pub offset: f64,
    pub min: f64,
    pub max: f64,
    pub unit: String,
}

impl Default for DbcData {
    fn default() -> Self {
        Self {
            file_path: String::new(),
            messages: Vec::new(),
            error_message: String::new(),
        }
    }
}

impl DbcData {
    pub fn new() -> Self {
        Default::default()
    }
    
    /// 加载 DBC 文件
    pub fn load_dbc_file<P: AsRef<Path>>(&mut self, path: P) -> Result<(), String> {
        let path = path.as_ref();
        self.file_path = path.to_string_lossy().to_string();
        
        // 检查文件是否存在
        if !path.exists() {
            return Err(format!("File not found: {}", path.display()));
        }
        
        // 读取文件内容
        let content = fs::read_to_string(path)
            .map_err(|e| format!("Failed to read file: {}", e))?;
        
        // 尝试解析 DBC 内容
        match can_dbc::DBC::from_slice(content.as_bytes()) {
            Ok(dbc) => {
                self.messages = self.extract_messages_safe(&dbc);
                self.error_message.clear();
                Ok(())
            }
            Err(e) => {
                // 清空之前的数据
                self.messages.clear();
                Err(format!("Failed to parse DBC file: {:?}", e))
            }
        }
    }
    
    /// 安全地从 DBC 中提取消息信息
    fn extract_messages_safe(&self, dbc: &can_dbc::DBC) -> Vec<MessageInfo> {
        let mut messages = Vec::new();
        
        for message in dbc.messages() {
            let mut signals = Vec::new();
            
            // 提取信号信息
            for signal in message.signals() {
                signals.push(SignalInfo {
                    name: signal.name().clone(),
                    start_bit: *signal.start_bit() as u8,
                    length: *signal.signal_size() as u8,
                    factor: *signal.factor(),
                    offset: *signal.offset(),
                    min: *signal.min(),
                    max: *signal.max(),
                    unit: signal.unit().clone(),
                });
            }
            
            // 提取消息信息
            messages.push(MessageInfo {
                id: message.message_id().raw(),
                name: message.message_name().clone(),
                length: *message.message_size() as u8,
                signals,
            });
        }
        
        messages
    }
    
    /// 获取消息数量
    pub fn message_count(&self) -> usize {
        self.messages.len()
    }
    
    /// 获取指定 ID 的消息
    pub fn get_message_by_id(&self, id: u32) -> Option<&MessageInfo> {
        self.messages.iter().find(|msg| msg.id == id)
    }
    
    /// 搜索消息（按名称）
    pub fn search_messages(&self, query: &str) -> Vec<&MessageInfo> {
        if query.is_empty() {
            return self.messages.iter().collect();
        }
        
        self.messages
            .iter()
            .filter(|msg| {
                msg.name.to_lowercase().contains(&query.to_lowercase())
                    || msg.signals.iter().any(|sig| {
                        sig.name.to_lowercase().contains(&query.to_lowercase())
                    })
            })
            .collect()
    }
}