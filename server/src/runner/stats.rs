use std::ops::Sub;

use serde::{Deserialize, Serialize};

/// cgroup stats of the container at a given point in time
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct CgroupStats {
    /// peak memory usage in bytes
    #[serde(rename(deserialize = "memory_bytes_peak"))]
    pub mem_peak: Option<usize>,
    /// total cpu time spent in user mode in microseconds
    #[serde(rename(deserialize = "cpu_user_us"))]
    pub cpu_user: Option<usize>,
    /// total cpu time spent in system mode in microseconds
    #[serde(rename(deserialize = "cpu_system_us"))]
    pub cpu_system: Option<usize>,
    /// total io bytes read
    #[serde(rename(deserialize = "io_total_bytes_read"))]
    pub io_tbr: Option<usize>,
    /// total io bytes written
    #[serde(rename(deserialize = "io_total_bytes_written"))]
    pub io_tbw: Option<usize>,
}

impl Sub for CgroupStats {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self {
            mem_peak: self.mem_peak.zip(rhs.mem_peak).map(|(lhs, rhs)| lhs.max(rhs)),
            cpu_user: self.cpu_user.zip(rhs.cpu_user).map(|(lhs, rhs)| lhs - rhs),
            cpu_system: self.cpu_system.zip(rhs.cpu_system).map(|(lhs, rhs)| lhs - rhs),
            io_tbr: self.io_tbr.zip(rhs.io_tbr).map(|(lhs, rhs)| lhs - rhs),
            io_tbw: self.io_tbw.zip(rhs.io_tbw).map(|(lhs, rhs)| lhs - rhs),
        }
    }
}
