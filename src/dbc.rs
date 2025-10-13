//! DBC 文件读取和解析模块
use std::fs;
use std::path::Path;
pub use can_dbc::{DBC, Message}; // 直接导出dbc库的类型

#[derive(Debug, Clone)]
pub struct DbcData {
    pub file_path: String,
    pub dbc: Option<DBC>, // 直接存储DBC对象
    pub error_message: String,
}

impl Default for DbcData {
    fn default() -> Self {
        Self {
            file_path: String::new(),
            dbc: None,
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
                self.dbc = Some(dbc);
                self.error_message.clear();
                Ok(())
            }
            Err(e) => {
                // 清空之前的数据
                self.dbc = None;
                Err(format!("Failed to parse DBC file: {:?}", e))
            }
        }
    }
    
    /// 获取DBC对象的引用
    #[allow(dead_code)]
    pub fn dbc(&self) -> Option<&DBC> {
        self.dbc.as_ref()
    }
    
    /// 获取消息数量
    pub fn message_count(&self) -> usize {
        self.dbc.as_ref().map_or(0, |dbc| dbc.messages().len())
    }
    
    /// 获取指定 ID 的消息
    pub fn get_message_by_id(&self, id: u32) -> Option<&Message> {
        self.dbc.as_ref()?.messages().iter()
            .find(|msg| msg.message_id().raw() == id)
    }
    
    /// 搜索消息（按名称）
    pub fn search_messages(&self, query: &str) -> Vec<&Message> {
        let Some(dbc) = self.dbc.as_ref() else {
            return Vec::new();
        };
        
        if query.is_empty() {
            return dbc.messages().iter().collect();
        }
        
        dbc.messages()
            .iter()
            .filter(|msg| {
                msg.message_name().to_lowercase().contains(&query.to_lowercase())
                    || msg.signals().iter().any(|sig| {
                        sig.name().to_lowercase().contains(&query.to_lowercase())
                    })
            })
            .collect()
    }
}