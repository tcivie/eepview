package io.github.tcivie.eepview.router;

import com.sun.net.httpserver.HttpExchange;
import com.sun.net.httpserver.HttpServer;
import java.io.IOException;
import java.io.OutputStream;
import java.net.InetAddress;
import java.net.InetSocketAddress;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import net.i2p.app.ClientAppManager;
import net.i2p.app.ClientAppState;
import net.i2p.router.RouterContext;
import net.i2p.router.app.RouterApp;
import net.i2p.util.Log;

public final class StatusApp implements RouterApp {
    static final String NAME = "eepview-status";
    private static final String LOOPBACK = "127.0.0.1";
    private static final String PATH = "/status";

    private final RouterContext ctx;
    private final ClientAppManager mgr;
    private final Log log;
    private final int port;
    private final Path tokenFile;
    private volatile ClientAppState state = ClientAppState.UNINITIALIZED;
    private HttpServer server;
    private ExecutorService executor;
    private byte[] token;

    public StatusApp(RouterContext ctx, ClientAppManager mgr, String[] args) {
        this.ctx = ctx;
        this.mgr = mgr;
        this.log = ctx.logManager().getLog(StatusApp.class);
        Args parsed = Args.parse(args);
        this.port = Integer.parseInt(parsed.require("port"));
        this.tokenFile = Path.of(parsed.require("tokenFile"));
        this.state = ClientAppState.INITIALIZED;
    }

    @Override
    public synchronized void startup() throws IOException {
        changeState(ClientAppState.STARTING, null, null);
        try {
            token = readToken(tokenFile);
            server = HttpServer.create(new InetSocketAddress(InetAddress.getByName(LOOPBACK), port), 8);
            executor = Executors.newSingleThreadExecutor(StatusApp::daemonThread);
            server.setExecutor(executor);
            server.createContext("/", this::handle);
            server.start();
        } catch (IOException | RuntimeException e) {
            changeState(ClientAppState.START_FAILED, "status endpoint failed to start", e);
            throw e;
        }
        changeState(ClientAppState.RUNNING, "listening on " + LOOPBACK + ':' + port, null);
        if (mgr != null) {
            mgr.register(this);
        }
    }

    @Override
    public synchronized void shutdown(String[] args) {
        if (state == ClientAppState.STOPPED) {
            return;
        }
        changeState(ClientAppState.STOPPING, null, null);
        if (server != null) {
            server.stop(0);
        }
        if (executor != null) {
            executor.shutdownNow();
        }
        changeState(ClientAppState.STOPPED, null, null);
        if (mgr != null) {
            mgr.unregister(this);
        }
    }

    @Override
    public ClientAppState getState() {
        return state;
    }

    @Override
    public String getName() {
        return NAME;
    }

    @Override
    public String getDisplayName() {
        return "eepview status endpoint";
    }

    private void handle(HttpExchange ex) throws IOException {
        try (ex) {
            respond(ex);
        }
    }

    private void respond(HttpExchange ex) throws IOException {
        int code = rejectCode(ex);
        if (code != 0) {
            send(ex, code, "{\"error\":" + code + "}");
            return;
        }
        String body;
        try {
            body = StatusJson.render(ctx);
        } catch (RuntimeException e) {
            log.error("status request failed", e);
            send(ex, 500, "{\"error\":500}");
            return;
        }
        send(ex, 200, body);
    }

    private int rejectCode(HttpExchange ex) {
        var headers = ex.getRequestHeaders();
        if (headers.containsKey("Origin")) {
            return 403;
        }
        if (!(LOOPBACK + ':' + port).equals(headers.getFirst("Host"))) {
            return 421;
        }
        if (!tokenMatches(headers.getFirst("Authorization"))) {
            return 401;
        }
        if (!PATH.equals(ex.getRequestURI().getRawPath()) || ex.getRequestURI().getRawQuery() != null) {
            return 404;
        }
        return "GET".equals(ex.getRequestMethod()) ? 0 : 405;
    }

    private boolean tokenMatches(String header) {
        String prefix = "Bearer ";
        if (header == null || !header.startsWith(prefix)) {
            return false;
        }
        byte[] given = header.substring(prefix.length()).getBytes(StandardCharsets.UTF_8);
        return MessageDigest.isEqual(given, token);
    }

    private static void send(HttpExchange ex, int code, String body) throws IOException {
        byte[] bytes = body.getBytes(StandardCharsets.UTF_8);
        var headers = ex.getResponseHeaders();
        headers.set("Content-Type", "application/json; charset=utf-8");
        headers.set("Cache-Control", "no-store");
        headers.set("X-Content-Type-Options", "nosniff");
        if (code == 405) {
            headers.set("Allow", "GET");
        }
        ex.sendResponseHeaders(code, bytes.length);
        try (OutputStream out = ex.getResponseBody()) {
            out.write(bytes);
        }
    }

    private static byte[] readToken(Path file) throws IOException {
        String value = Files.readString(file, StandardCharsets.UTF_8).trim();
        if (value.length() < 32) {
            throw new IOException("token in " + file + " is shorter than 32 characters");
        }
        return value.getBytes(StandardCharsets.UTF_8);
    }

    private static Thread daemonThread(Runnable r) {
        Thread t = new Thread(r, NAME);
        t.setDaemon(true);
        return t;
    }

    private void changeState(ClientAppState next, String msg, Exception e) {
        state = next;
        if (msg != null) {
            log.logAlways(e == null ? Log.INFO : Log.ERROR, NAME + ": " + msg + (e == null ? "" : ": " + e));
        }
        if (mgr != null) {
            mgr.notify(this, next, msg, e);
        }
    }
}
