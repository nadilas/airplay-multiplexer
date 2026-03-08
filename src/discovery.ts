import { EventEmitter } from 'events';
import { DeviceConfig, DeviceType } from './types';
import axios from 'axios';

export class DeviceDiscovery extends EventEmitter {
  private devices = new Map<string, DeviceConfig>();
  private sonosDiscovery: any;
  private ssdpClient: any;
  private bonjourBrowser: any;
  private bonjourInstance: any;
  private ssdpSearchInterval: ReturnType<typeof setInterval> | null = null;

  async start(): Promise<void> {
    console.log('[discovery] Starting device discovery...');
    this.startSonosDiscovery();
    this.startSsdpDiscovery();
    this.startBonjourDiscovery();
  }

  private startSonosDiscovery(): void {
    try {
      const { DeviceDiscovery: SonosDiscovery } = require('sonos');

      this.sonosDiscovery = new SonosDiscovery();

      this.sonosDiscovery.on('DeviceAvailable', (device: any) => {
        const host = device.host;
        const port = device.port || 1400;
        const id = `sonos-${host}`;

        if (this.devices.has(id)) return;

        // Get device name
        device.getName().then((name: string) => {
          const config: DeviceConfig = {
            id,
            name: name || `Sonos ${host}`,
            host,
            port,
            type: 'sonos',
          };

          this.addDevice(config);
        }).catch((err: any) => {
          // Fall back to IP-based name
          const config: DeviceConfig = {
            id,
            name: `Sonos ${host}`,
            host,
            port,
            type: 'sonos',
          };

          this.addDevice(config);
        });
      });

      console.log('[discovery] Sonos discovery started');
    } catch (err: any) {
      console.warn(`[discovery] Sonos discovery unavailable: ${err.message}`);
    }
  }

  private startSsdpDiscovery(): void {
    try {
      const { Client: SsdpClient } = require('node-ssdp');

      this.ssdpClient = new SsdpClient();

      this.ssdpClient.on('response', async (headers: any, statusCode: number, rinfo: any) => {
        if (statusCode !== 200) return;

        const location = headers.LOCATION;
        if (!location) return;

        const server = (headers.SERVER || '').toLowerCase();
        const st = headers.ST || '';

        // Look for MediaRenderer devices (Teufel, generic DLNA)
        if (st.includes('MediaRenderer') || st.includes('mediarenderer')) {
          await this.handleSsdpDevice(location, rinfo.address);
        }
      });

      // Search for DLNA MediaRenderers
      this.ssdpClient.search('urn:schemas-upnp-org:device:MediaRenderer:1');

      // Periodic re-search
      this.ssdpSearchInterval = setInterval(() => {
        this.ssdpClient.search('urn:schemas-upnp-org:device:MediaRenderer:1');
      }, 30000);

      console.log('[discovery] SSDP discovery started');
    } catch (err: any) {
      console.warn(`[discovery] SSDP discovery unavailable: ${err.message}`);
    }
  }

  private async handleSsdpDevice(location: string, host: string): Promise<void> {
    const id = `teufel-${host}`;
    if (this.devices.has(id)) return;

    try {
      const response = await axios.get(location, { timeout: 5000 });
      const xml = response.data;

      // Parse device name from XML description
      const nameMatch = xml.match(/<friendlyName>([^<]+)<\/friendlyName>/);
      const name = nameMatch ? nameMatch[1] : `DLNA Device ${host}`;

      // Check if it looks like a Teufel/Raumfeld or generic DLNA renderer
      const isTeufel = xml.toLowerCase().includes('teufel') || xml.toLowerCase().includes('raumfeld');
      const isMediaRenderer = xml.includes('MediaRenderer');

      if (isMediaRenderer) {
        const config: DeviceConfig = {
          id,
          name: isTeufel ? `Teufel ${name}` : name,
          host,
          port: new URL(location).port ? parseInt(new URL(location).port) : 80,
          type: 'teufel', // DLNA devices use the same UPnP control path
          location,
          model: isTeufel ? 'Teufel' : 'DLNA',
        };

        this.addDevice(config);
      }
    } catch (err: any) {
      // Could not fetch device description, skip
    }
  }

  private startBonjourDiscovery(): void {
    try {
      const { Bonjour } = require('bonjour-service');
      this.bonjourInstance = new Bonjour();

      this.bonjourBrowser = this.bonjourInstance.find({ type: 'airplay', protocol: 'tcp' });

      this.bonjourBrowser.on('up', (service: any) => {
        const host = service.referer?.address || service.addresses?.[0];
        if (!host) return;

        const id = `airplay-${host}`;
        if (this.devices.has(id)) return;

        const config: DeviceConfig = {
          id,
          name: service.name || `AirPlay ${host}`,
          host,
          port: service.port || 7000,
          type: 'airplay',
          model: service.txt?.model || service.txt?.am || undefined,
        };

        this.addDevice(config);
      });

      this.bonjourBrowser.on('down', (service: any) => {
        const host = service.referer?.address || service.addresses?.[0];
        if (!host) return;

        const id = `airplay-${host}`;
        this.removeDevice(id);
      });

      console.log('[discovery] Bonjour/AirPlay discovery started');
    } catch (err: any) {
      console.warn(`[discovery] Bonjour discovery unavailable: ${err.message}`);
    }
  }

  private addDevice(config: DeviceConfig): void {
    if (this.devices.has(config.id)) return;

    this.devices.set(config.id, config);
    console.log(`[discovery] Found ${config.type} device: ${config.name} (${config.host}:${config.port})`);
    this.emit('deviceFound', config);
  }

  private removeDevice(id: string): void {
    const config = this.devices.get(id);
    if (config) {
      this.devices.delete(id);
      console.log(`[discovery] Lost device: ${config.name}`);
      this.emit('deviceLost', config);
    }
  }

  getDevices(): DeviceConfig[] {
    return Array.from(this.devices.values());
  }

  getDevicesByType(type: DeviceType): DeviceConfig[] {
    return this.getDevices().filter((d) => d.type === type);
  }

  async stop(): Promise<void> {
    if (this.ssdpSearchInterval) {
      clearInterval(this.ssdpSearchInterval);
      this.ssdpSearchInterval = null;
    }

    if (this.ssdpClient) {
      try {
        this.ssdpClient.stop();
      } catch {
        // Ignore
      }
    }

    if (this.sonosDiscovery) {
      try {
        this.sonosDiscovery.destroy();
      } catch {
        // Ignore
      }
    }

    if (this.bonjourBrowser) {
      try {
        this.bonjourBrowser.stop();
      } catch {
        // Ignore
      }
    }

    if (this.bonjourInstance) {
      try {
        this.bonjourInstance.destroy();
      } catch {
        // Ignore
      }
    }

    this.devices.clear();
    console.log('[discovery] Discovery stopped');
  }
}
