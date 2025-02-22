import {
  ChildProcess,
  spawn,
} from 'child_process';
import { EventEmitter } from 'events';
import http from 'http';
import { Client as SSDPClient } from 'node-ssdp';
import { Sonos } from 'sonos';
import {
  PassThrough,
  Stream,
} from 'stream';
import { Device as UPNPDevice } from 'upnp-device-client';

interface DeviceConfig {
  name: string;
  host: string;
  port: number;
}

interface AudioMetadata {
  artist?: string;
  title?: string;
  album?: string;
  artwork?: Buffer;
}

class AudioDevice extends EventEmitter {
  protected volume: number = 50;
  protected isPlaying: boolean = false;
  protected isConnected: boolean = false;

  constructor(public name: string) {
    super();
  }

  async setVolume(volume: number): Promise<void> {
    this.volume = Math.max(0, Math.min(100, volume));
  }

  async play(stream: Stream): Promise<void> {
    throw new Error("Not implemented");
  }

  async stop(): Promise<void> {
    throw new Error("Not implemented");
  }

  async pause(): Promise<void> {
    throw new Error("Not implemented");
  }

  async resume(): Promise<void> {
    throw new Error("Not implemented");
  }

  getStatus(): { isPlaying: boolean; volume: number; isConnected: boolean } {
    return {
      isPlaying: this.isPlaying,
      volume: this.volume,
      isConnected: this.isConnected,
    };
  }
}

class SonosDevice extends AudioDevice {
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

class TeufelDevice extends AudioDevice {
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

class AudioMultiplexer extends EventEmitter {
  private devices: Map<string, AudioDevice> = new Map();
  private currentMetadata: AudioMetadata = {};
  private pipeStream: PassThrough;
  private readonly PIPE_PATH = "/tmp/shairport-sync-audio";
  private shairport?: ChildProcess;

  constructor() {
    super();
    this.pipeStream = new PassThrough();

    // Add error handler for the multiplexer itself
    this.on("error", this.handleProcessError.bind(this));
    this.on('fatalError', this.handleFatalError.bind(this));

    this.initializeShairport();
  }

  private initializeShairport() {
    try {
      // Create named pipe if it doesn't exist
      const fs = require("fs");
      if (!fs.existsSync(this.PIPE_PATH)) {
        const { execSync } = require("child_process");
        execSync(`mkfifo ${this.PIPE_PATH}`);
      }

      // Read from the pipe
      const { createReadStream } = require("fs");
      const pipeReader = createReadStream(this.PIPE_PATH);

      pipeReader.on("data", (chunk: Buffer) => {
        this.pipeStream.write(chunk);
        this.handleAudioStream(this.pipeStream);
      });

      pipeReader.on("error", (error: Error) => {
        this.handleProcessError(error);
      });

      // Start shairport-sync if not already running
      this.startShairportSync();
    } catch (error) {
      this.handleProcessError(error);
    }
  }

  private startShairportSync() {
    try {
      this.shairport = spawn("shairport-sync", [
        "-a",
        "Multi-Room Audio",
        "--output",
        "pipe",
        "--pipe-name",
        this.PIPE_PATH,
        "--metadata-pipename",
        "/tmp/shairport-sync-metadata",
      ]);

      this.shairport.stdout?.on("data", (data: Buffer) => {
        console.log("shairport-sync:", data.toString());
      });

      this.shairport.stderr?.on("data", (data: Buffer) => {
        this.handleProcessError(
          new Error(`Shairport error: ${data.toString()}`)
        );
      });

      this.shairport.on("error", (error: Error) => {
        this.handleProcessError(error);
      });

      this.shairport.on("close", (code: number) => {
        console.log("shairport-sync process exited with code:", code);
        if (code !== 0) {
          this.handleProcessError(
            new Error(`Shairport exited with code ${code}`)
          );
          this.restartShairportSync();
        }
      });

      process.on("exit", () => {
        this.shairport?.kill();
      });
    } catch (error) {
      this.handleProcessError(error);
    }
  }

  private restartShairportSync() {
    console.log("Attempting to restart shairport-sync...");
    setTimeout(() => {
      this.startShairportSync();
    }, 5000); // Wait 5 seconds before attempting restart
  }

  addDevice(id: string, device: AudioDevice): void {
    this.devices.set(id, device);
    device.on("error", (error) => this.emit("deviceError", { id, error }));
  }

  private async handleMetadata(meta: any) {
    this.currentMetadata = {
      artist: meta.artist,
      title: meta.title,
      album: meta.album,
      artwork: meta.artwork,
    };
    this.emit("metadata", this.currentMetadata);
  }

  private async handleAudioStream(stream: Stream) {
    try {
      const streams = this.createStreamCopies(stream, this.devices.size);

      const streamPromises = Array.from(this.devices.entries()).map(
        async ([id, device], index) => {
          try {
            await device.play(streams[index]);
          } catch (error) {
            console.error(`Failed to stream to device ${id}:`, error);
          }
        }
      );

      await Promise.all(streamPromises);
    } catch (error) {
      this.handleProcessError(error);
    }
  }

  private createStreamCopies(
    sourceStream: Stream,
    count: number
  ): PassThrough[] {
    const streams: PassThrough[] = [];
    for (let i = 0; i < count; i++) {
      const passThroughStream = new PassThrough();
      sourceStream.pipe(passThroughStream);
      streams.push(passThroughStream);
    }
    return streams;
  }

  async setMasterVolume(volume: number): Promise<void> {
    const volumePromises = Array.from(this.devices.values()).map((device) =>
      device.setVolume(volume)
    );
    await Promise.all(volumePromises);
  }

  private handleProcessError(error: Error): void {
    console.error("AudioMultiplexer encountered an error:", error);
    this.emit("error", error);

    // Attempt recovery
    try {
      this.restartShairportSync();
    } catch (recoveryError) {
      console.error("Recovery failed:", recoveryError);
    }
  }

  // Add a new event for fatal errors
  private async handleFatalError(error: Error): Promise<void> {
    console.error("Fatal error encountered:", error);
    try {
      await this.stop();
    } finally {
      // Notify that the multiplexer needs to be restarted
      process.exit(1);
    }
  }

  async stop(): Promise<void> {
    try {
      console.log("Stopping AudioMultiplexer...");

      // Stop all device streams
      const stopPromises = Array.from(this.devices.values()).map(
        async (device) => {
          try {
            await device.stop();
          } catch (error) {
            console.error(`Error stopping device ${device.name}:`, error);
          }
        }
      );

      await Promise.allSettled(stopPromises);

      // Cleanup pipe stream
      if (this.pipeStream) {
        this.pipeStream.end();
      }

      // Remove pipe file
      const fs = require("fs").promises;
      try {
        await fs.unlink(this.PIPE_PATH);
      } catch (error) {
        // Ignore if pipe doesn't exist
      }

      console.log("AudioMultiplexer stopped successfully");
    } catch (error) {
      console.error("Error during AudioMultiplexer shutdown:", error);
      throw error;
    }
  }

  // Add recovery method
  async recover(): Promise<void> {
    console.log("Attempting to recover AudioMultiplexer...");
    try {
      // Stop all current operations
      await this.stop();

      // Reinitialize
      this.pipeStream = new PassThrough();
      await this.initializeShairport();

      // Reconnect devices
      for (const device of this.devices.values()) {
        try {
          if (device instanceof SonosDevice || device instanceof TeufelDevice) {
            await device["initializeDevice"]();
          }
        } catch (error) {
          console.error(`Failed to reconnect device ${device.name}:`, error);
        }
      }

      console.log("Recovery completed successfully");
    } catch (error) {
      console.error("Recovery failed:", error);
      throw error;
    }
  }

  getStatus(): Record<string, any> {
    const status: Record<string, any> = {
      metadata: this.currentMetadata,
      devices: {},
    };

    for (const [id, device] of this.devices.entries()) {
      status.devices[id] = device.getStatus();
    }

    return status;
  }
}

// Usage example:
const multiplexer = new AudioMultiplexer();

// Add Sonos device
multiplexer.addDevice(
  "living-room-sonos",
  new SonosDevice({
    name: "Living Room Sonos",
    host: "192.168.1.x",
    port: 1400,
  })
);

// Add Teufel device
multiplexer.addDevice(
  "kitchen-teufel",
  new TeufelDevice({
    name: "Kitchen Teufel",
    host: "192.168.1.y",
    port: 1900,
  })
);

// Error handling
multiplexer.on("error", (error) => {
  console.error("Multiplexer error:", error);
});

multiplexer.on("deviceError", ({ id, error }) => {
  console.error(`Device ${id} error:`, error);
});

// Handle metadata updates
multiplexer.on("metadata", (metadata) => {
  console.log("Now playing:", metadata);
});

// Add error handling middleware
const handleDeviceError = async (id: string, error: Error) => {
  console.error(`Device ${id} error:`, error.message);
  try {
    const device = multiplexer["devices"].get(id);
    if (device) {
      // Attempt to reinitialize the device
      if (device instanceof SonosDevice || device instanceof TeufelDevice) {
        await device["initializeDevice"]();
      }
    }
  } catch (recoveryError) {
    console.error(`Failed to recover device ${id}:`, recoveryError.message);
  }
};

multiplexer.on("error", async (error: Error) => {
  console.error("Multiplexer error:", error.message);
  try {
    await multiplexer.recover();
  } catch (recoveryError) {
    console.error("Failed to recover from error:", recoveryError.message);
  }
});

// Handle fatal errors
multiplexer.on("fatalError", async (error: Error) => {
  console.error("Fatal error in AudioMultiplexer:", error.message);
  try {
    await multiplexer.stop();
  } finally {
    process.exit(1);
  }
});

multiplexer.on("deviceError", ({ id, error }) => {
  handleDeviceError(id, error).catch(console.error);
});

// Global error handlers
process.on("uncaughtException", (error: Error) => {
  console.error("Uncaught Exception:", error.message);
  console.error("Stack trace:", error.stack);
  // Optionally notify an error reporting service here

  // Gracefully shutdown
  (async () => {
    try {
      console.log("Attempting graceful shutdown...");
      await multiplexer.stop();
    } catch (shutdownError) {
      console.error("Error during shutdown:", shutdownError.message);
    } finally {
      // Force exit after 3 seconds if graceful shutdown fails
      setTimeout(() => {
        console.error("Forcing exit due to uncaught exception");
        process.exit(1);
      }, 3000);
    }
  })();
});

process.on("unhandledRejection", (reason: any, promise: Promise<any>) => {
  console.error("Unhandled Promise Rejection:");
  console.error("Promise:", promise);
  console.error("Reason:", reason);
  // Optionally notify an error reporting service here
});

// Warning handler
process.on("warning", (warning: Error) => {
  console.warn("Process Warning:", warning.name);
  console.warn("Message:", warning.message);
  console.warn("Stack:", warning.stack);
});

// Handle process signals
process.on("SIGTERM", async () => {
  console.log("Received SIGTERM signal");
  try {
    await multiplexer.stop();
    process.exit(0);
  } catch (error) {
    console.error("Error during SIGTERM shutdown:", error);
    process.exit(1);
  }
});

// Handle process termination
process.on("SIGINT", async () => {
  console.log("Stopping services...");
  await multiplexer.stop();
  process.exit(0);
});
