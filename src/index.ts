import { AudioMultiplexer } from './multiplexer';
import { SonosDevice } from './sonos';
import { TeufelDevice } from './teufel';

// Usage example:
const multiplexer = new AudioMultiplexer();

// Device discovery will happen automatically
// You can also check discovered devices:
setInterval(() => {
    const devices = multiplexer['deviceDiscovery'].getDiscoveredDevices();
    console.log('Currently discovered devices:', devices);
  }, 60000); // Every minute

// Add Teufel device
multiplexer.addDevice(
  "kitchen-teufel",
  new TeufelDevice({
    name: "Kitchen Teufel",
    host: "192.168.1.y",
    port: 1900,
    type: 'teufel'
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
