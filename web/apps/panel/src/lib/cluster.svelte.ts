// The live cluster: a snapshot from this device's service, then its stream of changes, reconnecting when it breaks.

import { createClient } from '@connectrpc/connect';
import { createConnectTransport } from '@connectrpc/connect-web';
import { ClusterService } from './gen/omnarc/v1/cluster_service_pb';
import { apply, emptyView, type ClusterView } from './model';

const transport = createConnectTransport({ baseUrl: location.origin, useBinaryFormat: true });
export const api = createClient(ClusterService, transport);

class LiveCluster {
  view = $state<ClusterView>(emptyView());
  /** Whether the stream from this device's service is open. */
  connected = $state(false);
  /** Ticks every second, so "online" and countdowns stay current. */
  now = $state(Date.now());
  private started = false;

  start(): void {
    if (this.started) return;
    this.started = true;
    setInterval(() => (this.now = Date.now()), 1_000);
    void this.watch();
  }

  private async watch(): Promise<void> {
    for (;;) {
      try {
        for await (const response of api.watchCluster({})) {
          this.connected = true;
          this.view = apply(this.view, response, Date.now());
        }
      } catch {
        // the service stopped or is restarting
      }
      this.connected = false;
      await new Promise((resolve) => setTimeout(resolve, 2_000));
    }
  }
}

export const cluster = new LiveCluster();
