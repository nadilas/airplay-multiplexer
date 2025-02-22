import { EventEmitter } from 'events';
import { Client as SSDPClient } from 'node-ssdp';
import {
  DeviceDiscovery as SonosDiscovery,
  Sonos,
} from 'sonos';
import { Stream } from 'stream';

import {
  AudioDevice,
  DeviceConfig,
} from './models';

export class DeviceDiscovery extends EventEmitter {
    private sonosDiscovery: any;
    private ssdpClient: SSDPClient;
    private discoveredDevices: Map<string, DeviceConfig> = new Map();
  
    constructor() {
      super();
      this.ssdpClient = new SSDPClient();
      this.initializeDiscovery();
    }
  
    private initializeDiscovery() {
      // Initialize Sonos discovery
      SonosDiscovery((device: any) => {
        const config: DeviceConfig = {
          name: `Sonos ${device.host}`,
          host: device.host,
          port: 1400,
          type: 'sonos'
        };
        this.discoveredDevices.set(`sonos-${device.host}`, config);
        this.emit('deviceFound', config);
      });
  
      // Discover Teufel/DLNA devices
      this.ssdpClient.on('response', (headers: any) => {
        if (headers.ST?.includes('urn:schemas-upnp-org:device:MediaRenderer:1')) {
          const location = new URL(headers.LOCATION);
          const config: DeviceConfig = {
            name: headers['SERVER'] || `Teufel ${location.hostname}`,
            host: location.hostname,
            port: parseInt(location.port) || 1900,
            type: 'teufel'
          };
          this.discoveredDevices.set(`teufel-${location.hostname}`, config);
          this.emit('deviceFound', config);
        }
      });
    }
  
    async startDiscovery(): Promise<void> {
      try {
        // Start SSDP discovery
        this.ssdpClient.search('urn:schemas-upnp-org:device:MediaRenderer:1');
  
        // Repeat SSDP search periodically
        setInterval(() => {
          this.ssdpClient.search('urn:schemas-upnp-org:device:MediaRenderer:1');
        }, 30000); // Every 30 seconds
      } catch (error) {
        console.error('Error starting device discovery:', error.message);
      }
    }
  
    getDiscoveredDevices(): DeviceConfig[] {
      return Array.from(this.discoveredDevices.values());
    }
  
    stop(): void {
      this.sonosDiscovery?.stop();
      this.ssdpClient?.stop();
    }
  }

  export class SonosDevice extends AudioDevice {
    private device: Sonos;
  
    constructor(config: DeviceConfig) {
      super(config.name);
      this.device = new Sonos(config.host, config.port);
      this.initializeDevice();
    }
  
    private async initializeDevice() {
      try {
        const deviceInfo = await this.device.getMusicLibrary("queue");
        this.isConnected = true;
        this.emit("connected");
      } catch (error) {
        console.error(`Failed to initialize Sonos device ${this.name}:`, error.message);
        this.emit("error", error);
      }
    }
  
    async setVolume(volume: number): Promise<void> {
      try {
        await super.setVolume(volume);
        await this.device.setVolume(this.volume);
      } catch (error) {
        console.error(
          `Failed to set volume on Sonos device ${this.name}:`,
          error
        );
        this.emit("error", error);
      }
    }
  
    async play(stream: Stream): Promise<void> {
      try {
        await this.device.play(stream);
        this.isPlaying = true;
        this.emit("playing");
      } catch (error) {
        console.error(`Failed to play on Sonos device ${this.name}:`, error.message);
        this.emit("error", error);
      }
    }
  
    async stop(): Promise<void> {
      try {
        await this.device.stop();
        this.isPlaying = false;
        this.emit("stopped");
      } catch (error) {
        console.error(`Failed to stop Sonos device ${this.name}:`, error.message);
        this.emit("error", error);
      }
    }
  }