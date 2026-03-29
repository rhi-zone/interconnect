use std::path::PathBuf;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

use crate::protocol::{Request, Response};

/// Connect to the daemon socket and send a single request, returning the
/// parsed response.
pub async fn send_request(socket_path: &PathBuf, request: &Request) -> anyhow::Result<Response> {
    let stream = UnixStream::connect(socket_path).await.map_err(|e| {
        anyhow::anyhow!(
            "could not connect to daemon at {}: {}\n\
             Hint: start the daemon with `interconnect-daemon`",
            socket_path.display(),
            e
        )
    })?;

    let (reader, mut writer) = stream.into_split();
    let mut lines = BufReader::new(reader).lines();

    let mut payload = serde_json::to_string(request)?;
    payload.push('\n');
    writer.write_all(payload.as_bytes()).await?;
    // Signal that we're done writing so the daemon can read EOF if needed.
    drop(writer);

    let line = lines
        .next_line()
        .await?
        .ok_or_else(|| anyhow::anyhow!("daemon closed connection without responding"))?;

    let response: Response = serde_json::from_str(&line)?;
    Ok(response)
}
