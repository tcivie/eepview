package io.github.tcivie.eepview.router;

import net.i2p.router.RouterContext;
import net.i2p.router.RouterVersion;
import net.i2p.router.TunnelManagerFacade;
import net.i2p.router.transport.FIFOBandwidthLimiter;
import net.i2p.stat.Rate;
import net.i2p.stat.RateStat;

final class StatusJson {
    private static final long TEN_MINUTES = 600_000L;

    private StatusJson() {}

    static String render(RouterContext ctx) {
        StringBuilder sb = new StringBuilder(512).append('{');
        field(sb, "version", quote(RouterVersion.FULL_VERSION));
        field(sb, "uptimeMs", ctx.router().getUptime());
        field(sb, "networkStatus", quote(ctx.commSystem().getStatus().name()));
        field(sb, "knownRouters", ctx.netDb().getKnownRouters());
        field(sb, "activePeers", ctx.commSystem().countActivePeers());
        field(sb, "tunnels", tunnels(ctx.tunnelManager()));
        field(sb, "bandwidthBytesPerSecond", bandwidth(ctx.bandwidthLimiter()));
        field(sb, "tunnelBuildSuccessPercent", buildSuccess(ctx));
        sb.setLength(sb.length() - 1);
        return sb.append('}').toString();
    }

    private static String tunnels(TunnelManagerFacade tm) {
        StringBuilder sb = new StringBuilder("{");
        field(sb, "clientInbound", tm.getInboundClientTunnelCount());
        field(sb, "clientOutbound", tm.getOutboundClientTunnelCount());
        field(sb, "exploratoryInbound", tm.getFreeTunnelCount());
        field(sb, "exploratoryOutbound", tm.getOutboundTunnelCount());
        field(sb, "participating", tm.getParticipatingCount());
        return close(sb);
    }

    private static String bandwidth(FIFOBandwidthLimiter bw) {
        StringBuilder sb = new StringBuilder("{");
        field(sb, "in1s", Math.round(bw.getReceiveBps()));
        field(sb, "out1s", Math.round(bw.getSendBps()));
        field(sb, "in15s", Math.round(bw.getReceiveBps15s()));
        field(sb, "out15s", Math.round(bw.getSendBps15s()));
        return close(sb);
    }

    private static String buildSuccess(RouterContext ctx) {
        StringBuilder sb = new StringBuilder("{");
        field(sb, "exploratory", successPercent(ctx, "tunnel.buildExploratory"));
        field(sb, "client", successPercent(ctx, "tunnel.buildClient"));
        return close(sb);
    }

    private static String successPercent(RouterContext ctx, String prefix) {
        long ok = recentEvents(ctx, prefix + "Success");
        long total = ok + recentEvents(ctx, prefix + "Expire") + recentEvents(ctx, prefix + "Reject");
        return total == 0 ? "null" : Long.toString(100 * ok / total);
    }

    private static long recentEvents(RouterContext ctx, String name) {
        RateStat stat = ctx.statManager().getRate(name);
        Rate rate = stat == null ? null : stat.getRate(TEN_MINUTES);
        return rate == null ? 0 : rate.getCurrentEventCount() + rate.getLastEventCount();
    }

    private static void field(StringBuilder sb, String key, Object value) {
        sb.append('"').append(key).append("\":").append(value).append(',');
    }

    private static String close(StringBuilder sb) {
        sb.setLength(sb.length() - 1);
        return sb.append('}').toString();
    }

    private static String quote(String s) {
        StringBuilder sb = new StringBuilder(s.length() + 2).append('"');
        for (char c : s.toCharArray()) {
            if (c == '"' || c == '\\') {
                sb.append('\\').append(c);
            } else if (c >= 0x20) {
                sb.append(c);
            }
        }
        return sb.append('"').toString();
    }
}
