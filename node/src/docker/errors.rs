use std::fmt;

#[derive(Debug)]
pub enum DockerError {
    ConnectionError(String),
    ImagePullError(String),
    ContainerCreationError(String),
    ContainerStartError(String),
    ContainerInspectError(String),
    ContainerRemovalError(String),
    ContainerStopError(String),
    LogsError(String),
    StreamLogsError(String),
}

impl fmt::Display for DockerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DockerError::ConnectionError(msg) => write!(f, "Connection error: {}", msg),
            DockerError::ImagePullError(msg) => write!(f, "Image pull error: {}", msg),
            DockerError::ContainerCreationError(msg) => {
                write!(f, "Container creation error: {}", msg)
            }
            DockerError::ContainerStartError(msg) => write!(f, "Container start error: {}", msg),
            DockerError::ContainerRemovalError(msg) => {
                write!(f, "Container removal error: {}", msg)
            }
            DockerError::ContainerStopError(msg) => write!(f, "Container stop error: {}", msg),
            DockerError::ContainerInspectError(msg) => {
                write!(f, "Container inspect error: {}", msg)
            }
            DockerError::LogsError(msg) => write!(f, "Logs error: {}", msg),
            DockerError::StreamLogsError(msg) => write!(f, "Stream logs error: {}", msg),
        }
    }
}
