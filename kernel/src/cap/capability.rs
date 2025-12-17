use super::CapType;
use super::rights;

/// 能力 (Capability)
/// 包含对象引用、权限和 Badge
#[derive(Debug, Clone, Copy)]
pub struct Capability {
    pub object: CapType,
    pub badge: Option<usize>, // Badge 用于服务端识别客户端身份
    pub rights: u8,
}

impl Capability {
    pub const fn new(object: CapType, rights: u8) -> Self {
        Self { object, badge: None, rights }
    }

    pub const fn empty() -> Self {
        Self { object: CapType::Empty, badge: None, rights: 0 }
    }

    /// Mint 操作：创建一个新的 Cap，可以附加 Badge
    pub fn mint(&self, badge: Option<usize>) -> Self {
        Self {
            object: self.object,
            badge: badge.or(self.badge), // 如果已有 Badge 则保留，否则使用新的
            rights: self.rights,
        }
    }

    /// 检查是否拥有指定权限
    pub fn has_rights(&self, required: u8) -> bool {
        (self.rights & required) == required
    }

    /// 检查是否允许 Invoke (Call)
    pub fn can_invoke(&self) -> bool {
        self.has_rights(rights::CALL)
    }

    /// 检查是否允许 Grant (传递)
    pub fn can_grant(&self) -> bool {
        self.has_rights(rights::GRANT)
    }
}
