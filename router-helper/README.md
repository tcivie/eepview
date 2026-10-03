# router-helper (spike S9)

A small jar that runs inside a headless Java I2P router (2.13.0) whose web
console is switched off. It gives eepview what the console gave before:

- `StatusApp` — a read-only status endpoint, `GET /status`, on 127.0.0.1.
- `AddressBookApp` — the address book subscription daemon, without susidns.

The router's own update and news managers run without the console too. They
need no code here, only a `clients.config.d` entry each (see below).

## Build

```sh
router-helper/build.sh /path/to/i2p/base   # the dir that holds lib/router.jar
```

The jar lands in `router-helper/out/eepview-router-helper.jar`. It needs only
the JDK (17 or later) and the router's own jars. It has no other dependency.

## Run

Put the jar on the router's classpath, next to `lib/*.jar`. Then add one file
per app to `<config>/clients.config.d/`. Each file uses the prefix `clientApp.0.`.

```properties
# 01-io.github.tcivie.eepview.router.StatusApp-clients.config
clientApp.0.main=io.github.tcivie.eepview.router.StatusApp
clientApp.0.name=eepview status
clientApp.0.args=port=37600 tokenFile=/abs/path/status.token
clientApp.0.delay=0
clientApp.0.startOnLoad=true
```

| `main` class | `args` |
|---|---|
| `net.i2p.i2ptunnel.TunnelControllerGroup` | `i2ptunnel.config` (`delay=-1`) |
| `io.github.tcivie.eepview.router.StatusApp` | `port=<n> tokenFile=<path>` |
| `io.github.tcivie.eepview.router.AddressBookApp` | `home=addressbook` (optional) |
| `net.i2p.router.update.ConsoleUpdateManager` | none |
| `net.i2p.router.news.NewsManager` | none |

Do not add `net.i2p.router.web.RouterConsoleRunner`. Without it, no console
port opens and Jetty never starts.

## The status endpoint

```sh
curl -H "Authorization: Bearer $(cat status.token)" http://127.0.0.1:37600/status
```

The response holds the router version, the uptime, the network status, the
known routers, the active peers, the tunnel counts, the bandwidth (1 s and
15 s), and the tunnel build success rate. The build success rate is `null`
until the router has sampled it once.

The endpoint checks each request in this order:

| Check | Reply | Why |
|---|---|---|
| An `Origin` header is present | 403 | Browsers send it. No web page may read the endpoint. |
| `Host` is not `127.0.0.1:<port>` | 421 | Stops DNS rebinding. |
| The bearer token is missing or wrong | 401 | The compare runs in constant time. |
| The path is not `/status` | 404 | Checked after auth, so a caller without the token learns nothing. |
| The method is not `GET` | 405 | The endpoint is read-only. |

The token file must hold at least 32 characters. Create it with mode 0600:
`(umask 077; openssl rand -hex 32 > status.token)`. The endpoint sends no CORS
headers.

## Things to set up before the first start

- **Copy `hosts.txt` into the config dir.** The router copies the base files
  (`hosts.txt` among them) into a new config dir only when that dir holds none
  of them yet (`WorkingDir.isSetup`). A config dir that you seed with
  `router.config` first is "already set up", so it gets no `hosts.txt`. The
  naming service then starts with zero entries and `.i2p` names do not resolve.
- **Point the address book at your proxy.** `addressbook/config.txt` must set
  `proxy_port` to the HTTP proxy port of this router. The daemon defaults to
  4444, which is the port of a normal I2P install.
- **Seed `addressbook/subscriptions.txt`** with the susidns default:
  `http://i2p-projekt.i2p/hosts.txt` and `http://notbob.i2p/hosts.txt`.
- **Point the update manager at your proxy.** In `router.config`, set
  `router.updateProxyPort` to the HTTP proxy port.

## Timings

- The address book waits 5 to 10 minutes after start, then fetches every
  `update_delay` hours (default 12).
- The first news check runs 5 to 10 minutes after start, or 25 to 30 minutes
  on a router installed less than 30 minutes ago. These delays are constants
  in `NewsTimerTask`. No property shortens them.
