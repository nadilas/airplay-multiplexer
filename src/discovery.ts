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
  private discoveryInterval?: NodeJS.Timeout;

  constructor() {
    super();

    console.log("Initializing device discovery...");
    this.ssdpClient = new SSDPClient();
    this.bonjour = new Bonjour();
    this.initializeDiscovery();
  }

  private initializeDiscovery() {
    console.log("Setting up discovery listeners...");

    // Add SSDP search listeners
    this.ssdpClient.on("advertise-alive", (headers: any) => {
      console.log("SSDP alive advertisement received:", headers.NT);
    });

    this.ssdpClient.on("advertise-bye", (headers: any) => {
      console.log("SSDP bye advertisement received:", headers.NT);
    });

    this.initializeSonosDiscovery();
    this.initializeTeufelDiscovery();
    this.initializeAirplayDiscovery();
  }

  private initializeSonosDiscovery() {
    console.log("Initializing Sonos discovery...");
    SonosDiscovery((device: any) => {
      const config: DeviceConfig = {
        name: `Sonos ${device.host}`,
        host: device.host,
        port: 1400,
        type: "sonos",
      };
      const deviceId = `sonos-${device.host}`;
      this.addDevice(deviceId, config);
    });
  }

  private initializeTeufelDiscovery() {
    console.log("Initializing Teufel/DLNA discovery...");
    this.ssdpClient.on("response", async (headers: any) => {
      console.log("SSDP device details:", {
        server: headers.SERVER,
        location: headers.LOCATION,
        st: headers.ST,
      });
      try {
        // Teufel/Raumfeld devices might not advertise in SERVER header
        // Also check the location and ST (search target) headers
        const isTeufel =
          headers.SERVER?.toLowerCase().includes("teufel") ||
          headers.SERVER?.toLowerCase().includes("raumfeld") ||
          headers.LOCATION?.toLowerCase().includes("teufel") ||
          headers.LOCATION?.toLowerCase().includes("raumfeld") ||
          headers.ST?.toLowerCase().includes("teufel") ||
          headers.ST?.toLowerCase().includes("raumfeld");

        if (isTeufel) {
          console.log("Teufel device found:", headers.LOCATION);
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
            type: "teufel",
            location: headers.LOCATION,
            serviceType: description.serviceType,
          };

          const deviceId = `teufel-${config.host}`;
          this.addDevice(deviceId, config);
        }
      } catch (error) {
        console.error("Error handling Teufel device discovery:", error.message);
      }
    });
  }
  private initializeAirplayDiscovery() {
    console.log("Initializing Airplay discovery...");
    // Browse for AirPlay 2 devices
    const browser = this.bonjour.find({
      type: "airplay",
      protocol: "tcp",
    });

    browser.on("up", (service) => {
      console.log("Bonjour service details:", {
        name: service.name,
        type: service.type,
        txt: service.txt,
        host: service.host,
      });

      const isAirplay = service.txt && service.txt.model
      if (isAirplay) {
        console.log("AirPlay device found:", service.name);
        const config: DeviceConfig = {
          name: service.name,
          host: service.host,
          port: service.port || 7000,
          type: "airplay",
          features: {
            airplay2: true,
            model: service.txt.model,
            deviceId: service.txt.deviceid,
            features: service.txt.features,
            flags: service.txt.flags
          },
        };

        const deviceId = `airplay-${service.txt.deviceid || service.host}`;
        this.addDevice(deviceId, config);
      }
    });

    browser.on("down", (service) => {
      console.log("Bonjour service lost:", service.name);
      if (service.txt && service.txt.id) {
        const deviceId = `airplay-${service.txt.id}`;
        this.removeDevice(deviceId);
      }
    });
  }

  private addDevice(id: string, config: DeviceConfig) {
    if (!this.discoveredDevices.has(id)) {
      this.discoveredDevices.set(id, config);
      this.emit("deviceFound", config);
    }
  }

  private removeDevice(id: string) {
    if (this.discoveredDevices.has(id)) {
      const device = this.discoveredDevices.get(id);
      this.discoveredDevices.delete(id);
      this.emit("deviceLost", device);
    }
  }

  private startSSDPClient() {
    try {
      this.ssdpClient.start();
      console.log("SSDP client started successfully");
    } catch (error) {
      console.error("Failed to start SSDP client:", error);
    }
  }

  async startDiscovery(): Promise<void> {
    try {
      console.log("Starting active device discovery...");

      // Start SSDP client explicitly
      this.startSSDPClient();

      // initial search
      this.performSearch();

      // Repeat SSDP search periodically
      this.discoveryInterval = setInterval(() => {
        this.performSearch();
      }, 30000);

      // Log current state after a short delay
      setTimeout(() => {
        this.logDiscoveryStatus();
      }, 5000);
    } catch (error) {
      console.error("Error starting device discovery:", error.message);
    }
  }

  private performSearch() {
    console.log("Performing device search...");
    const searches = [
      "urn:schemas-upnp-org:device:MediaRenderer:1",
      "urn:schemas-teufel-systems:device:*",
      "urn:schemas-raumfeld:device:*",
    ];

    searches.forEach((search) => {
      console.log(`Searching for: ${search}`);
      this.ssdpClient.search(search);
    });
  }

  private logDiscoveryStatus() {
    console.log("\n=== Discovery Status ===");
    console.log("SSDP Client active:", !!(this.ssdpClient as any)._bound);
    console.log("Bonjour Browser active:", !!this.bonjour);
    console.log("Discovered devices:", this.discoveredDevices.size);
    console.log(
      "Device types found:",
      new Set([...this.discoveredDevices.values()].map((d) => d.type))
    );
    console.log("=====================\n");
  }

  stop(): void {
    console.log("Stopping device discovery...");
    if (this.discoveryInterval) {
      clearInterval(this.discoveryInterval);
    }
    this.ssdpClient?.stop();
    this.bonjour?.destroy();
    console.log("Device discovery stopped.");
  }

  getDiscoveredDevices(): DeviceConfig[] {
    return Array.from(this.discoveredDevices.values());
  }

  getDiscoveryStatus(): Record<string, any> {
    return {
      totalDevices: this.discoveredDevices.size,
      devicesByType: {
        sonos: this.getDevicesByType("sonos").length,
        teufel: this.getDevicesByType("teufel").length,
        airplay: this.getDevicesByType("airplay").length,
      },
      ssdpActive: !!(this.ssdpClient as any)._bound,
      bonjourActive: !!this.bonjour,
      discoveredDevices: Array.from(this.discoveredDevices.entries()).map(
        ([id, config]) => ({
          id,
          name: config.name,
          type: config.type,
          host: config.host,
        })
      ),
    };
  }

  getDevicesByType(type: "sonos" | "teufel" | "airplay"): DeviceConfig[] {
    return this.getDiscoveredDevices().filter((device) => device.type === type);
  }
}
