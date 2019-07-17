quick_error! {
    #[derive(Debug)]
    pub enum FSError {
        SystemError(errno: nix::errno::Errno) {
            description(errno.desc())
        }
        InvalidPath
        InvalidUtf8
    }
}

impl From<nix::Error> for FSError {
    fn from(e: nix::Error) -> Self {
        match e {
            nix::Error::Sys(errno) => FSError::SystemError(errno),
            nix::Error::InvalidPath => FSError::InvalidPath,
            nix::Error::InvalidUtf8 => FSError::InvalidUtf8,
            nix::Error::UnsupportedOperation => unreachable!(),
        }
    }
}

pub type Result<T> = std::result::Result<T, FSError>;
