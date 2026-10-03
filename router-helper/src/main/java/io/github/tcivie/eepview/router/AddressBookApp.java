package io.github.tcivie.eepview.router;

import net.i2p.addressbook.DaemonThread;
import net.i2p.app.ClientAppManager;
import net.i2p.app.ClientAppState;
import net.i2p.router.RouterContext;
import net.i2p.router.app.RouterApp;
import net.i2p.util.Log;

public final class AddressBookApp implements RouterApp {
    static final String NAME = "eepview-addressbook";

    private final ClientAppManager mgr;
    private final Log log;
    private final String home;
    private volatile ClientAppState state = ClientAppState.UNINITIALIZED;
    private DaemonThread daemon;

    public AddressBookApp(RouterContext ctx, ClientAppManager mgr, String[] args) {
        this.mgr = mgr;
        this.log = ctx.logManager().getLog(AddressBookApp.class);
        this.home = Args.parse(args).get("home", "addressbook");
        this.state = ClientAppState.INITIALIZED;
    }

    @Override
    public synchronized void startup() {
        changeState(ClientAppState.STARTING);
        daemon = new DaemonThread(new String[] {home});
        daemon.setName(NAME);
        daemon.setDaemon(true);
        daemon.start();
        changeState(ClientAppState.RUNNING);
        log.logAlways(Log.INFO, NAME + ": address book daemon started, home=" + home);
        if (mgr != null) {
            mgr.register(this);
        }
    }

    @Override
    public synchronized void shutdown(String[] args) {
        if (state == ClientAppState.STOPPED) {
            return;
        }
        changeState(ClientAppState.STOPPING);
        if (daemon != null) {
            daemon.halt();
        }
        changeState(ClientAppState.STOPPED);
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
        return "eepview address book";
    }

    private void changeState(ClientAppState next) {
        state = next;
        if (mgr != null) {
            mgr.notify(this, next, null, null);
        }
    }
}
