#!/usr/bin/env python3
import argparse
import json
import os
import pathlib
import socket
import sqlite3
import ssl
import struct
import subprocess
import time
import urllib.request

class E2EError(RuntimeError):
    pass

def rpc(url, method, params=None):
    body=json.dumps({"jsonrpc":"2.0","id":1,"method":method,"params":params or []}).encode()
    req=urllib.request.Request(url, body, {"Content-Type":"application/json"})
    with urllib.request.urlopen(req, timeout=3) as response:
        value=json.load(response)
    if value.get("error"):
        raise E2EError(f"RPC {method} failed: {value['error']}")
    return value.get("result")

def wait_for(fn, label, timeout=45):
    end=time.time()+timeout
    last=None
    while time.time()<end:
        try:
            return fn()
        except Exception as exc:
            last=exc
            time.sleep(1)
    raise E2EError(f"timeout waiting for {label}: {last}")

def message(kind, **fields):
    return json.dumps({kind:fields}, separators=(",",":")).encode()

def send_frame(sock, body):
    sock.sendall(struct.pack(">I",len(body))+body)

def recv_frame(sock):
    header=sock.recv(4)
    if len(header)!=4:
        raise E2EError("relay closed before frame header")
    size=struct.unpack(">I",header)[0]
    body=b""
    while len(body)<size:
        part=sock.recv(size-len(body))
        if not part:
            raise E2EError("relay closed during frame")
        body+=part
    return json.loads(body)

def relay_session(cert_dir, host, port, node_id="clap-provider-1"):
    context=ssl.create_default_context(ssl.Purpose.SERVER_AUTH, cafile=str(cert_dir/"ca.crt"))
    context.check_hostname=False
    context.load_cert_chain(str(cert_dir/"client.crt"), str(cert_dir/"client.key"))
    raw=socket.create_connection((host,port), timeout=5)
    sock=context.wrap_socket(raw, server_hostname="kette12-relay")
    send_frame(sock,message("Handshake",node_id=node_id,role="e2e-orchestrator"))
    ack=recv_frame(sock)
    if ack.get("Ack",{}).get("status")!="ok":
        raise E2EError(f"handshake rejected: {ack}")
    return sock

def publish(cert_dir, host, port, topic, data):
    sock=relay_session(cert_dir,host,port)
    try:
        send_frame(sock,message("Payload",topic=topic,data=list(data)))
        ack=recv_frame(sock)
        status=ack.get("Ack",{}).get("status","")
        if not status.startswith("published:"):
            raise E2EError(f"payload rejected: {ack}")
        return status
    finally:
        sock.close()

def db_counts(path):
    with sqlite3.connect(path, timeout=3) as db:
        pending=db.execute("select count(*) from outbox_events where published_at is null").fetchone()[0]
        published=db.execute("select count(*) from outbox_events where published_at is not null").fetchone()[0]
        return pending,published

def compose(args, *extra):
    command=["docker","compose","-f",args.compose,*extra]
    return subprocess.run(command, check=True, text=True, capture_output=True)

def main():
    parser=argparse.ArgumentParser(description="Omega Nexus mTLS/outbox/Anvil E2E runner")
    parser.add_argument("--compose",default="docker-compose.yml")
    parser.add_argument("--cert-dir",default="arche-omega-relayer/certs")
    parser.add_argument("--db",default=".e2e-data/outbox.sqlite3")
    parser.add_argument("--relay-host",default="127.0.0.1")
    parser.add_argument("--relay-port",type=int,default=8080)
    parser.add_argument("--rpc-url",default="http://127.0.0.1:8545")
    parser.add_argument("--load",type=int,default=25)
    parser.add_argument("--host-recovery",action="store_true",help="also exercise relay restart and Anvil RPC outage")
    args=parser.parse_args()
    cert_dir=pathlib.Path(args.cert_dir)
    db=pathlib.Path(args.db)
    wait_for(lambda: urllib.request.urlopen("http://127.0.0.1:9090/",timeout=2),"relay health")
    chain=wait_for(lambda: rpc(args.rpc_url,"eth_chainId"),"Anvil JSON-RPC")
    print(f"READY relay=mtls anvil_chain={chain}")
    before=db_counts(db)
    status=publish(cert_dir,args.relay_host,args.relay_port,"clap.embedding.request",b"dry-run-payload")
    print(f"PAYLOAD status={status}")
    wait_for(lambda: db_counts(db)[0]==0,"dry-run outbox drain")
    after=db_counts(db)
    if after[1] <= before[1]:
        raise E2EError("dry-run event was not marked published")
    for index in range(args.load):
        publish(cert_dir,args.relay_host,args.relay_port,"clap.embedding.request",f"load-{index}".encode())
    wait_for(lambda: db_counts(db)[0]==0,"load outbox drain")
    print(f"LOAD passed={args.load} counts={db_counts(db)}")
    if args.host_recovery:
        compose(args,"stop","relay")
        compose(args,"up","-d","relay")
        wait_for(lambda: urllib.request.urlopen("http://127.0.0.1:9090/",timeout=2),"relay restart")
        publish(cert_dir,args.relay_host,args.relay_port,"clap.embedding.request",b"restart-recovery")
        wait_for(lambda: db_counts(db)[0]==0,"outbox recovery after relay restart")
        print("RESTART recovery=passed")
        compose(args,"stop","anvil")
        publish(cert_dir,args.relay_host,args.relay_port,"clap.embedding.request",b"rpc-outage")
        time.sleep(3)
        pending,_=db_counts(db)
        if pending==0:
            raise E2EError("RPC outage did not leave event pending")
        compose(args,"up","-d","anvil")
        wait_for(lambda: rpc(args.rpc_url,"eth_chainId"),"Anvil RPC recovery")
        wait_for(lambda: db_counts(db)[0]==0,"outbox retry after RPC recovery",60)
        print("RPC outage/recovery=passed")
    print("E2E PASS")

if __name__=="__main__":
    try:
        main()
    except Exception as exc:
        print(f"E2E FAIL: {exc}")
        raise SystemExit(1)
