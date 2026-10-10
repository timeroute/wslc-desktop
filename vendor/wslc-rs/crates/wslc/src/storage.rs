/// VHD volume type.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VhdType {
    /// Expanding VHDX.
    Dynamic,
    /// Fixed-allocation VHDX.
    Fixed,
}

impl VhdType {
    pub(crate) fn as_raw(self) -> wslc_sys::WslcVhdType {
        match self {
            Self::Dynamic => wslc_sys::WslcVhdType::WSLC_VHD_TYPE_DYNAMIC,
            Self::Fixed => wslc_sys::WslcVhdType::WSLC_VHD_TYPE_FIXED,
        }
    }
}

/// VHD-backed session volume options.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VhdOptions {
    /// Volume name.
    pub name: String,
    /// Desired size in bytes.
    pub size_bytes: u64,
    /// VHD allocation strategy.
    pub vhd_type: VhdType,
    /// Optional Linux owner.
    pub owner: Option<(u32, u32)>,
}

impl VhdOptions {
    /// Creates volume options.
    pub fn new(name: impl Into<String>, size_bytes: u64) -> Self {
        Self {
            name: name.into(),
            size_bytes,
            vhd_type: VhdType::Dynamic,
            owner: None,
        }
    }

    /// Sets the VHD type.
    pub fn vhd_type(mut self, vhd_type: VhdType) -> Self {
        self.vhd_type = vhd_type;
        self
    }

    /// Sets the Linux owner for create-volume operations.
    pub fn owner(mut self, uid: u32, gid: u32) -> Self {
        self.owner = Some((uid, gid));
        self
    }
}
