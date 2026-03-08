import { AudioFormat } from './types';
import os from 'os';

export interface AppConfig {
  receiverName: string;
  httpPort: number;
  audioFormat: AudioFormat;
  shairportPath: string;
  localIp: string;
}

function getLocalIp(): string {
  const interfaces = os.networkInterfaces();
  for (const name of Object.keys(interfaces)) {
    const addrs = interfaces[name];
    if (!addrs) continue;
    for (const addr of addrs) {
      if (addr.family === 'IPv4' && !addr.internal) {
        return addr.address;
      }
    }
  }
  return '127.0.0.1';
}

export function loadConfig(): AppConfig {
  return {
    receiverName: process.env.RECEIVER_NAME || 'Multi-Room Audio',
    httpPort: parseInt(process.env.HTTP_PORT || '5000', 10),
    audioFormat: {
      sampleRate: parseInt(process.env.SAMPLE_RATE || '44100', 10),
      bitDepth: parseInt(process.env.BIT_DEPTH || '16', 10),
      channels: parseInt(process.env.CHANNELS || '2', 10),
    },
    shairportPath: process.env.SHAIRPORT_PATH || 'shairport-sync',
    localIp: process.env.LOCAL_IP || getLocalIp(),
  };
}
