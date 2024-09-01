# nrpctl
Nginx Reverse Proxy Control

A simple tool to set up nginx reverse proxies.

Basic usage:
```sh
nrpctl status
nrpctl add [--https] [--www] something.mydomain.com http://localhost:1234
nrpctl remove something.mydomain.com
```

Sets up an nginx reverse proxy redirecting from the given domain to another domain and port.
By default, listens only on port 80.
If `--https` is passed, then use `certbot` to generate an SSL certificate registered to the subdomain, and listens on both port 80 and 443, automatically redirecting any http traffic to https instead.
If `--www` is passed, then listen for `www.something.mydomain.com` as well as `something.mydomain.com`.

## Back-of-the-napkin design

(literally)

### Pieces:
- CLI to get status and add, remove, or modify existing proxies
- Library to build nginx config files based on the current state (idempotent)
- Library to restart/reload nginx itself based on the current state
  - Should vary based on whether the program is running as a snap or as a standalone program on a system with nginx
- Library to manage SSL certificates via certbot
  - Should also vary based on whether it's running in a snap or not
  - Easy solution is to use the certbot snap, but that's classic
  - Less easy solution is to use the certbot python package, but that requires some additional interactivity
  - Might make the most sense to just run certbot manually and place the certificates in the correct location and modify the nginx configurations accordingly
- Library to parse and write state
- Probably need some sort of validation that we don't have conflicting port listeners
 - E.g. there doesn't exist another application listening on a port that one of the nginx reverse proxies will be listening on
   - We probably should only support listening on port 80 and 443, with an automatic redirect from 80 to 443 before we reach the proxied service
 - In a snap install, need to make sure that a system-wide install of nginx does not conflict with the nginx included in this snap

### Config Files:
- Likely use environment variables to specify where the nginx sites available via `nrpctl` live
- Regardless, these are someplace special, not in `/etc/nginx/sites-available`, so we can easily and idempotently ensure that the current nginx configuration files match what is in the application state
- On non-snap installs, we create symlinks from wherever the `nrpctl` sites-available live into `/etc/nginx/sites-enabled`, so they integrate with existing nginx
- On snap installs, both nginx and certbot should run within the snap, and use sandboxed config file and certificate locations

### Structure:
- `snap/snapcraft.yaml`
- `src/`
  - `main.rs`
  - `cli.rs` (encapsulate CLI parsing)
  - `state.rs` (store state of the reverse proxies so we can rebuild nginx config files idempotently)
  - `nginx.rs` (start/reload nginx, either the system install or the snap binary wrappers -- maybe not necessary)
  - `certbot.rs` (run certbot, either the system install or the snap binary wrapper, for any sites which don't have certs)
- `bin/` (called from the used by the snap `app` attributes)
  - `start-nginx` (run the nginx daemon, since we can't use the system nginx)
  - `restart-nginx` (reload the nginx configuration and restart nginx)
  - `run-certbot` (run certbot manually, since we don't have access to the classic snap)
