import EventEmitter from 'events';
import { ShairportSync } from 'shairport-sync';
import {
  PassThrough,
  Stream,
} from 'stream';

import { AirplayDevice } from './airplay';
import { DeviceDiscovery } from './discovery';
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
  private shairport?: ShairportSync;

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
    console.log("Starting device discovery...");

    this.deviceDiscovery.on("deviceFound", (config: DeviceConfig) => {
      console.log(`Found device: ${config.name} at ${config.host}`);

      // Automatically add discovered devices
      switch (config.type) {
        case "sonos":
          this.addDevice(`sonos-${config.host}`, new SonosDevice(config));
          break;
        case "teufel":
          this.addDevice(`teufel-${config.host}`, new TeufelDevice(config));
          break;
        case "homepod":
          this.addDevice(`homepod-${config.host}`, new AirplayDevice(config));
          break;
      }
    });

    this.deviceDiscovery.on("deviceLost", (config: DeviceConfig) => {
      console.log(`Lost device: ${config.name}`);
    });

    // Start discovery and set up status monitoring
    this.deviceDiscovery
      .startDiscovery()
      .then(() => {
        // Check discovery status every 10 seconds for the first minute
        let checks = 0;
        const statusInterval = setInterval(() => {
          checks++;
          const status = this.deviceDiscovery.getDiscoveryStatus();
          console.log("\nDiscovery Status Update:", status);

          if (checks >= 6) {
            // After 1 minute
            clearInterval(statusInterval);
            if (status.totalDevices === 0) {
              console.warn(
                "No devices found after 1 minute. Please check your network configuration."
              );
            }
          }
        }, 10000);
      })
      .catch((error) => {
        console.error("Failed to start device discovery:", error.message);
      });
  }

  private initializeShairport() {
    try {
        this.startShairportSync();
    } catch (error) {
      this.handleProcessError(error);
    }
  }

  private startShairportSync() {
    const airplay = new ShairportSync();

    // Set the receiver public name
    airplay.name = 'Multi-Room Audio';

    airplay.start()

    airplay.output.stream.pipe(this.pipeStream);
    airplay.output.stream.on('data', (chunk: Buffer) => {
        console.log('pipe data', chunk)
      this.handleAudioStream(this.pipeStream);
    });
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
            console.log('playing stream on device', id)
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
    console.log("stop() called from:", stack?.split("\n").slice(2).join("\n"));

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
