use ssh2::Session as SshSession;
use std::net::TcpStream;

pub enum SshEvent {
    HostKeyVerify {
        host: String,
        port: u16,
        fingerprint: String,
        response: flume::Sender<bool>,
    }
}

pub fn connect_ssh(
    host: &str, 
    port: u16, 
    user: &str, 
    pass: &str, 
    key_path: Option<&str>, 
    keepalive: u32, 
    agent_forwarding: bool, 
    term_type: &str,
    event_tx: Option<flume::Sender<SshEvent>>
) -> Result<(ssh2::Session, ssh2::Channel), Box<dyn std::error::Error + Send + Sync>> {
    let tcp = TcpStream::connect(format!("{}:{}", host, port))?;
    let mut sess = SshSession::new()?;
    sess.set_tcp_stream(tcp);
    sess.handshake()?;

    if let Some(etx) = event_tx {
        let hash = sess.host_key_hash(ssh2::HashType::Sha256).ok_or("Could not get host key hash")?;
        let fingerprint = hash.iter().map(|b| format!("{:02x}", b)).collect::<String>();
        
        let known_hosts = crate::config::load_known_hosts();
        let is_known = known_hosts.iter().any(|h| h.host == host && h.port == port && h.fingerprint == fingerprint);
        
        if !is_known {
            let (tx, rx) = flume::bounded(1);
            let _ = etx.send(SshEvent::HostKeyVerify {
                host: host.to_string(),
                port,
                fingerprint: fingerprint.clone(),
                response: tx,
            });
            
            if !rx.recv().unwrap_or(false) {
                return Err("Host key verification failed".into());
            }
        }
    }
    
    if let Some(path) = key_path {
        sess.userauth_pubkey_file(user, None, std::path::Path::new(path), None)?;
    } else if !pass.is_empty() {
        sess.userauth_password(user, pass)?;
    } else {
        return Err("Authentication failed: No password or key specified".into());
    }

    if keepalive > 0 {
        sess.set_keepalive(true, keepalive);
    }
    
    let mut channel = sess.channel_session()?;
    if agent_forwarding {
        let _ = channel.request_auth_agent_forwarding();
    }
    channel.request_pty(term_type, None, Some((80, 24, 0, 0)))?;
    channel.shell()?;
    Ok((sess, channel))
}
