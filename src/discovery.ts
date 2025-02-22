import Bonjour from 'bonjour-service';
// src/DeviceDiscovery.ts
import { EventEmitter } from 'events';
import { Client as SSDPClient } from 'node-ssdp';
import { DeviceDiscovery as SonosDiscovery } from 'sonos';
import { Device as UPNPDevice } from 'upnp-device-client';

import { DeviceConfig } from './models';

export class DeviceDiscovery extends EventEmitter {
  private ssdpClient: SSDPClient;
  private bonjour: Bonjour;
  private discoveredDevices: Map<string, DeviceConfig> = new Map();

  constructor() {
    super();
    this.ssdpClient = new SSDPClient();
    this.bonjour = new Bonjour();
    this.initializeDiscovery();
  }

  private initializeDiscovery() {
    this.initializeSonosDiscovery();
    this.initializeTeufelDiscovery();
    this.initializeHomePodDiscovery();
  }

  private initializeSonosDiscovery() {
    SonosDiscovery((device: any) => {
      const config: DeviceConfig = {
        name: `Sonos ${device.host}`,
        host: device.host,
        port: 1400,
        type: 'sonos'
      };
      const deviceId = `sonos-${device.host}`;
      this.addDevice(deviceId, config);
    });
  }

  private initializeTeufelDiscovery() {
    this.ssdpClient.on('response', async (headers: any) => {
      try {
        if (headers.SERVER?.toLowerCase().includes('teufel') ||
            headers.SERVER?.toLowerCase().includes('raumfeld')) {
          
          const device = new UPNPDevice(headers.LOCATION);
          const description = await new Promise((resolve, reject) => {
            device.getDeviceDescription((err: Error, desc: any) => {
              if (err) reject(err);
              else resolve(desc);
            });
          });

          const config: DeviceConfig = {
            name: description.friendlyName || `Teufel ${headers.LOCATION}`,
            host: new URL(headers.LOCATION).hostname,
            port: parseInt(new URL(headers.LOCATION).port) || 1900,
            type: 'teufel',
            location: headers.LOCATION,
            serviceType: description.serviceType
          };

          const deviceId = `teufel-${config.host}`;
          this.addDevice(deviceId, config);
        }
      } catch (error) {
        console.error('Error handling Teufel device discovery:', error.message);
      }
    });
  }
  private initializeHomePodDiscovery() {
    // Browse for AirPlay 2 devices
    const browser = this.bonjour.find({
      type: 'airplay',
      protocol: 'tcp'
    });

    browser.on('up', (service) => {
      // Check if it's a HomePod
      if (service.txt && (
          service.txt.am === 'HomePod' || 
          service.txt.md === 'HomePod' ||
          service.name.toLowerCase().includes('homepod'))) {
        
        const config: DeviceConfig = {
          name: service.name,
          host: service.host,
          port: service.port,
          type: 'homepod',
          features: {
            airplay2: true,
            audioFormats: service.txt.sf?.split(',') || [],
            bonjourId: service.txt.id,
            model: service.txt.md,
            manufacturer: service.txt.am
          }
        };

        const deviceId = `homepod-${service.txt.id}`;
        this.addDevice(deviceId, config);
      }
    });

    browser.on('down', (service) => {
      if (service.txt && service.txt.id) {
        const deviceId = `homepod-${service.txt.id}`;
        this.removeDevice(deviceId);
      }
    });
  }

  private addDevice(id: string, config: DeviceConfig) {
    if (!this.discoveredDevices.has(id)) {
      this.discoveredDevices.set(id, config);
      this.emit('deviceFound', config);
    }
  }

  private removeDevice(id: string) {
    if (this.discoveredDevices.has(id)) {
      const device = this.discoveredDevices.get(id);
      this.discoveredDevices.delete(id);
      this.emit('deviceLost', device);
    }
  }

  async startDiscovery(): Promise<void> {
    try {
        // Search for UPNP/DLNA devices
        this.ssdpClient.search('urn:schemas-upnp-org:device:MediaRenderer:1');
        this.ssdpClient.search('urn:schemas-teufel-systems:device:*');
  
        // Repeat SSDP search periodically
        setInterval(() => {
          this.ssdpClient.search('urn:schemas-upnp-org:device:MediaRenderer:1');
          this.ssdpClient.search('urn:schemas-teufel-systems:device:*');
        }, 30000);
  
      } catch (error) {
        console.error('Error starting device discovery:', error.message);
      }
  }

  stop(): void {
    this.ssdpClient?.stop();
    this.bonjour?.destroy();
  }

  getDiscoveredDevices(): DeviceConfig[] {
    return Array.from(this.discoveredDevices.values());
  }

  getDevicesByType(type: 'sonos' | 'teufel' | 'homepod'): DeviceConfig[] {
    return this.getDiscoveredDevices().filter(device => device.type === type);
  }
}