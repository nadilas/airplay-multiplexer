import http from 'http';
import { Client as SSDPClient } from 'node-ssdp';
import { Stream } from 'stream';
import { Device as UPNPDevice } from 'upnp-device-client';

import {
  AudioDevice,
  DeviceConfig,
} from './models';

export class TeufelDevice extends AudioDevice {
  private ssdpClient: SSDPClient;
  private upnpDevice?: UPNPDevice;
  private streamServer?: http.Server;
  private readonly DLNA_SERVICE = "urn:schemas-upnp-org:service:AVTransport:1";

  constructor(config: DeviceConfig) {
    super(config.name);
    this.ssdpClient = new SSDPClient();
    this.initializeDevice(config.host);
  }

  private async initializeDevice(host: string) {
    try {
      // Search for DLNA devices
      this.ssdpClient.on("response", async (headers: any) => {
        if (headers.LOCATION.includes(host)) {
          this.upnpDevice = new UPNPDevice(headers.LOCATION);
          this.isConnected = true;
          this.emit("connected");
        }
      });

      this.ssdpClient.search("urn:schemas-upnp-org:device:MediaRenderer:1");
    } catch (error) {
      console.error(`Failed to initialize Teufel device ${this.name}:`, error.message);
      this.emit("error", error);
    }
  }

  async setVolume(volume: number): Promise<void> {
    try {
      await super.setVolume(volume);
      if (this.upnpDevice) {
        await this.upnpDevice.callAction("RenderingControl", "SetVolume", {
          InstanceID: 0,
          Channel: "Master",
          DesiredVolume: this.volume,
        });
      }
    } catch (error) {
      console.error(
        `Failed to set volume on Teufel device ${this.name}:`,
        error.message
      );
      this.emit("error", error);
    }
  }

  async play(stream: Stream): Promise<void> {
    try {
      const streamUrl = await this.setupStreamServer(stream);
      if (this.upnpDevice) {
        await this.upnpDevice.callAction("AVTransport", "SetAVTransportURI", {
          InstanceID: 0,
          CurrentURI: streamUrl,
          CurrentURIMetaData: "",
        });
        await this.upnpDevice.callAction("AVTransport", "Play", {
          InstanceID: 0,
          Speed: "1",
        });
        this.isPlaying = true;
        this.emit("playing");
      }
    } catch (error) {
      console.error(`Failed to play on Teufel device ${this.name}:`, error.message);
      this.emit("error", error);
    }
  }

  async stop(): Promise<void> {
    try {
      if (this.upnpDevice) {
        await this.upnpDevice.callAction("AVTransport", "Stop", {
          InstanceID: 0,
        });
      }
      if (this.streamServer) {
        this.streamServer.close();
      }
      this.isPlaying = false;
      this.emit("stopped");
    } catch (error) {
      console.error(`Failed to stop Teufel device ${this.name}:`, error.message);
      this.emit("error", error);
    }
  }

  private async setupStreamServer(stream: Stream): Promise<string> {
    return new Promise((resolve, reject) => {
      try {
        // Create HTTP server to stream audio
        this.streamServer = http.createServer((req, res) => {
          res.writeHead(200, {
            "Content-Type": "audio/wav",
            "Transfer-Encoding": "chunked",
          });
          stream.pipe(res);
        });

        // Get local IP address
        const { networkInterfaces } = require("os");
        const nets = networkInterfaces();
        let localIp = "";
        for (const name of Object.keys(nets)) {
          for (const net of nets[name]) {
            if (net.family === "IPv4" && !net.internal) {
              localIp = net.address;
              break;
            }
          }
          if (localIp) break;
        }

        // Start server on random port
        this.streamServer.listen(0, () => {
          const port = (this.streamServer!.address() as any).port;
          const streamUrl = `http://${localIp}:${port}/stream`;
          resolve(streamUrl);
        });

        this.streamServer.on("error", (error) => {
          reject(error);
        });
      } catch (error) {
        reject(error);
      }
    });
  }
}