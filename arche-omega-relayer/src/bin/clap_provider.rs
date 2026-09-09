use rustls::{Certificate,ClientConfig,PrivateKey,RootCertStore,ServerName};
use rustls_pemfile::{certs,pkcs8_private_keys};
use serde::{Deserialize,Serialize};
use std::{env,fs::File,io::BufReader,sync::Arc};
use tokio::{io::{AsyncReadExt,AsyncWriteExt},net::TcpStream,time::{sleep,Duration}};
use tokio_rustls::TlsConnector;
#[derive(Serialize,Deserialize,Debug)]
#[serde(tag="type",rename_all="PascalCase")]
enum M{Handshake{node_id:String,role:String},Heartbeat{timestamp:u64},Payload{topic:String,data:Vec<u8>},Ack{status:String}}
fn v(k:&str,d:&str)->String{env::var(k).unwrap_or_else(|_|d.into())}
fn cert(p:&str)->Vec<Certificate>{certs(&mut BufReader::new(File::open(p).unwrap())).unwrap().into_iter().map(Certificate).collect()}
fn key(p:&str)->PrivateKey{PrivateKey(pkcs8_private_keys(&mut BufReader::new(File::open(p).unwrap())).unwrap().remove(0))}
async fn put<S:AsyncWriteExt+Unpin>(s:&mut S,m:&M)->Result<(),Box<dyn std::error::Error+Send+Sync>>{let b=serde_json::to_vec(m)?;s.write_all(&(b.len()as u32).to_be_bytes()).await?;s.write_all(&b).await?;s.flush().await?;Ok(())}
async fn get<S:AsyncReadExt+Unpin>(s:&mut S)->Result<M,Box<dyn std::error::Error+Send+Sync>>{let mut h=[0;4];s.read_exact(&mut h).await?;let mut b=vec![0;u32::from_be_bytes(h)as usize];s.read_exact(&mut b).await?;Ok(serde_json::from_slice(&b)?) }
#[tokio::main]
async fn main()->Result<(),Box<dyn std::error::Error+Send+Sync>>{let mut r=RootCertStore::empty();for c in cert(&v("RELAY_CA_CERT","certs/ca.crt")){r.add(&c)?;}let c=ClientConfig::builder().with_safe_defaults().with_root_certificates(r).with_client_auth_cert(cert(&v("CLAP_CLIENT_CERT","certs/client.crt")),key(&v("CLAP_CLIENT_KEY","certs/client.key")))?;let t=TlsConnector::from(Arc::new(c));let x=TcpStream::connect(v("RELAY_ADDR","127.0.0.1:8080")).await?;let n=ServerName::try_from(v("RELAY_SERVER_NAME","localhost")).map_err(|_|"bad server name")?.to_owned();let mut s=t.connect(n,x).await?;put(&mut s,&M::Handshake{node_id:v("CLAP_NODE_ID","clap-provider"),role:"clap-provider".into()}).await?;match get(&mut s).await?{M::Ack{status}if status=="ok"=>{},x=>return Err(format!("handshake rejected: {x:?}").into())};put(&mut s,&M::Payload{topic:"clap.embedding.request".into(),data:b"provider-ready".to_vec()}).await?;eprintln!("ack: {:?}",get(&mut s).await?);loop{put(&mut s,&M::Heartbeat{timestamp:0}).await?;let _=get(&mut s).await?;sleep(Duration::from_secs(15)).await}}
