
pub struct ZeroReader {
    length: u64,
    current: u64,
}

impl ZeroReader {
    pub fn new(length : u64) -> ZeroReader {
        ZeroReader{
            length,
            current: 0,
        }
    }
}

impl std::io::Read for ZeroReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let remaining_bytes = self.length - self.current;
        println!("Reading {remaining_bytes} zeros");
        if remaining_bytes == 0 {
            return Ok(0);
        }
        let read_len = if buf.len() < (remaining_bytes as usize) {
            buf.len()
        } else {
            remaining_bytes as usize
        };
        for i in &mut buf[..read_len] {
            // The '0' character
            *i = 48;
        }

        Ok(read_len)
    }
}

impl std::io::Seek for ZeroReader {
    fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
        match pos {
            std::io::SeekFrom::Start(v) => {
                self.current = v;
            }
            std::io::SeekFrom::End(v) => {
                if v <= 0 {
                    let inv_v = -v as u64;
                    if inv_v > self.length {
                        return Err(std::io::Error::from(std::io::ErrorKind::InvalidInput));
                    }
                    self.current = self.length - inv_v;
                } else {
                    self.current = self.length + v as u64;
                }
            },
            std::io::SeekFrom::Current(v) => {
                if v <= 0 {
                    let inv_v = -v as u64;
                    if inv_v > self.current {
                        return Err(std::io::Error::from(std::io::ErrorKind::InvalidInput));
                    }
                    self.current = self.length - inv_v;
                } else {
                    self.current += v as u64;
                }
            },
        }
        Ok(self.current)
    }
}
