# Henchman

Run tasks on a server via a lightweight HTTP interface. 

You could think of it as a very simple Jenkins — not for doing builds, but for all those other little tasks like triggering deployments or other custom scripts.

This is a work-in-progress, but hopefully still useful.

# Disclaimer

While every attempt has been made to ensure this software is secure, it is intended for use on internal networks only (eg, over a VPN) and not battle-tested enough to be exposed publicly on the internet.

# Usage

```
RUST_LOG=info henchman [...]
```
