import {
  ChildProcess,
  spawn,
} from 'child_process';
import EventEmitter from 'events';
import {
  PassThrough,
  Stream,
} from 'stream';

import { DeviceDiscovery } from './discovery';
import { HomePodDevice } from './homepod';
import {
  AudioDevice,
  DeviceConfig,
} from './models';
import { SonosDevice } from './sonos';
import { TeufelDevice } from './teufel';

interface AudioMetadata {
  artist?: string;
  title?: string;
  album?: string;
  artwork?: Buffer;
}

export class AudioMultiplexer extends EventEmitter {
  private devices: Map<string, AudioDevice> = new Map();
  private deviceDiscovery: DeviceDiscovery;
  private currentMetadata: AudioMetadata = {};
  private pipeStream: PassThrough;
  private readonly PIPE_PATH = "/tmp/shairport-sync-audio";
  private shairport?: ChildProcess;

  constructor() {
    super();
    this.pipeStream = new PassThrough();
    this.deviceDiscovery = new DeviceDiscovery();
    this.initializeDeviceDiscovery();

    // Add error handler for the multiplexer itself
    this.on("error", this.handleProcessError.bind(this));
    this.on("fatalError", this.handleFatalError.bind(this));

    this.initializeShairport();
  }

  private initializeDeviceDiscovery() {
    this.deviceDiscovery.on("deviceFound", (config: DeviceConfig) => {
        console.log(`Found device: ${config.name} at ${config.host}`);
    
        // Automatically add discovered devices
        switch(config.type) {
          case "sonos":
            this.addDevice(`sonos-${config.host}`, new SonosDevice(config));
            break;
          case "teufel":
            this.addDevice(`teufel-${config.host}`, new TeufelDevice(config));
            break;
          case "homepod":
            this.addDevice(`homepod-${config.host}`, new HomePodDevice(config));
            break;
        }
      });
    
      this.deviceDiscovery.on("deviceLost", (config: DeviceConfig) => {
        console.log(`Lost device: ${config.name}`);
        // Handle device removal if needed
      });
    
      this.deviceDiscovery.startDiscovery().catch((error) => {
        console.error("Failed to start device discovery:", error.message);
      });
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
      // First check if shairport-sync is installed
      try {
        const { execSync } = require("child_process");
        execSync("which shairport-sync");
      } catch (error) {
        this.emit(
          "fatalError",
          new Error("shairport-sync is not installed. Please install it first.")
        );
        return;
      }

      // Check if shairport-sync is already running
      try {
        const { execSync } = require("child_process");
        const runningProcess = execSync("pgrep shairport-sync").toString();
        if (runningProcess) {
          execSync(`kill ${runningProcess}`);
          console.log("Killed existing shairport-sync process");
        }
      } catch (error) {
        // No existing process found, which is fine
      }

      console.log("Starting shairport-sync with options:", {
        name: "Multi-Room Audio",
        pipePath: this.PIPE_PATH,
      });

      // Updated command line options
      this.shairport = spawn("shairport-sync", [
        "-a",
        "Multi-Room Audio", // Set AirPlay name
        "-p",
        "6000",
        "-d",
        "-o",
        "stdout", // Output to stdout instead of pipe
        "-v", // Verbose mode
      ]);

      // Pipe shairport's output to our named pipe
      if (this.shairport.stdout) {
        const fs = require("fs");
        const writeStream = fs.createWriteStream(this.PIPE_PATH);
        this.shairport.stdout.pipe(writeStream);
      }

      let startupBuffer = "";
      const startupTimeout = setTimeout(() => {
        console.error("Startup buffer content:", startupBuffer);
        this.handleProcessError(new Error("shairport-sync startup timeout"));
      }, 5000);

      this.shairport.stdout?.on("data", (data: Buffer) => {
        const message = data.toString();
        startupBuffer += message;
        console.log("shairport-sync:", message);
      });

      this.shairport.stderr?.on("data", (data: Buffer) => {
        const errorMsg = data.toString();
        startupBuffer += errorMsg;

        // Only treat actual errors as errors
        if (
          errorMsg.toLowerCase().includes("error") ||
          errorMsg.toLowerCase().includes("fatal")
        ) {
          if (errorMsg.includes("daemon") || errorMsg.includes("running")) {
            this.emit(
              "fatalError",
              new Error("shairport-sync daemon conflict: " + errorMsg)
            );
            return;
          }

          if (errorMsg.includes("permission")) {
            this.emit(
              "fatalError",
              new Error("shairport-sync permission error: " + errorMsg)
            );
            return;
          }

          console.error("shairport-sync error:", errorMsg);
          this.handleProcessError(new Error(`Shairport error: ${errorMsg}`));
        } else {
          // Just log non-error messages
          console.log("shairport-sync:", errorMsg);
        }
      });

      this.shairport.on("error", (error: Error) => {
        clearTimeout(startupTimeout);
        console.error("Shairport process error:", error.message);
        this.handleProcessError(error);
      });

      let restartAttempts = 0;
      const MAX_RESTART_ATTEMPTS = 3;

      this.shairport.on("close", (code: number) => {
        clearTimeout(startupTimeout);
        console.log("shairport-sync process exited with code:", code);

        if (code !== 0) {
          if (restartAttempts < MAX_RESTART_ATTEMPTS) {
            restartAttempts++;
            console.log(
              `Restart attempt ${restartAttempts} of ${MAX_RESTART_ATTEMPTS}`
            );
            this.restartShairportSync();
          } else {
            this.emit(
              "fatalError",
              new Error(
                `Shairport failed to start after ${MAX_RESTART_ATTEMPTS} attempts. Last error: ${startupBuffer}`
              )
            );
          }
        }
      });

      process.on("exit", () => {
        if (this.shairport?.pid) {
          try {
            process.kill(this.shairport.pid);
          } catch (error) {
            // Ignore kill errors during shutdown
          }
        }
      });
    } catch (error) {
      console.error("Failed to start shairport-sync:", error);
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
    // Add stack trace logging
    const stack = new Error().stack;
    console.log('stop() called from:', stack?.split('\n').slice(2).join('\n'));

    try {
      console.log("Stopping AudioMultiplexer...");
      this.deviceDiscovery.stop();

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
