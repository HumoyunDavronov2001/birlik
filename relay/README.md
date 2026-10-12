# omnarc-relay

Devices of one cluster on different networks — a phone on mobile data, a laptop at home, a desktop at the office —
reach each other through a relay (`protocol/CLUSTER.md` §3.5). The relay helps both sides punch a direct path through
their NATs, which works about nine times in ten; when it does not, the relay carries their traffic. It is end-to-end
encrypted between the devices: the relay sees which devices talk, never what they say.

Omnarc uses no public relay unless you name one. In stage B2 the relay is the official
[iroh-relay](https://github.com/n0-computer/iroh) server, configured for Omnarc; WebRTC signaling and the public
directory come with stage B5.

## Run it

You need a server with a public address and a DNS name pointing at it (here `relay.example.com`), and open ports:
TCP 80 (the Let's Encrypt challenge), TCP 443 (the relay) and UDP 7842 (address discovery for hole punching).

```bash
cargo install iroh-relay --version 1.3.0 --features server --locked
sudo install -m 755 ~/.cargo/bin/iroh-relay /usr/local/bin/

sudo mkdir -p /etc/omnarc-relay
sudo cp relay/omnarc-relay.toml /etc/omnarc-relay/        # set hostname and contact in it
echo "IROH_RELAY_ACCESS_TOKEN=$(openssl rand -hex 24)" | sudo tee /etc/omnarc-relay/token.env >/dev/null
sudo chmod 600 /etc/omnarc-relay/token.env

sudo cp relay/omnarc-relay.service /etc/systemd/system/
sudo systemctl enable --now omnarc-relay
```

## Use it

The relay's address carries the token, so only your clusters use it:

```bash
omnarc-node create --name "Home cluster" --relay "https://relay.example.com?token=<the token>"
# or, for a cluster that exists
omnarc-node relay add "https://relay.example.com?token=<the token>"
```

Tickets name the relay with its token, so a device on another network can join; it then uses the cluster's relay
from its first start. `omnarc-node relay list` shows the relays without their tokens.

To try it on one machine, `iroh-relay --dev` runs a relay over plain HTTP on `http://localhost:3340`.
