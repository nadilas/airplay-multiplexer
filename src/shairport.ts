import { ChildProcess, spawn } from 'child_process';
import { EventEmitter } from 'events';
import { Readable } from 'stream';
import { AppConfig } from './config';
import { TrackMetadata } from './types';

export class ShairportManager extends EventEmitter {
  private process: ChildProcess | null = null;
  private config: AppConfig;
  private restarting = false;
  private restartAttempts = 0;
  private maxRestartAttempts = 10;
  private restartDelay = 2000;

  constructor(config: AppConfig) {
    super();
    this.config = config;
  }

  async start(): Promise<void> {
    await this.validateBinary();
    this.spawnProcess();
  }

  private async validateBinary(): Promise<void> {
    return new Promise((resolve, reject) => {
      const check = spawn(this.config.shairportPath, ['--version'], {
        stdio: ['ignore', 'pipe', 'pipe'],
      });

      let output = '';
      check.stdout?.on('data', (d) => (output += d.toString()));
      check.stderr?.on('data', (d) => (output += d.toString()));

      check.on('close', (code) => {
        // shairport-sync --version may exit with 0 or 1 depending on version
        if (output.toLowerCase().includes('shairport')) {
          console.log(`[shairport] Found: ${output.trim()}`);
          resolve();
        } else if (code === 0) {
          resolve();
        } else {
          reject(
            new Error(
              `shairport-sync binary not found or not working at "${this.config.shairportPath}". ` +
                `Install it with: sudo apt install shairport-sync (Linux) or brew install shairport-sync (macOS). ` +
                `Output: ${output}`
            )
          );
        }
      });

      check.on('error', (err) => {
        reject(
          new Error(
            `shairport-sync binary not found at "${this.config.shairportPath}". ` +
              `Install it with: sudo apt install shairport-sync (Linux) or brew install shairport-sync (macOS). ` +
              `Error: ${err.message}`
          )
        );
      });
    });
  }

  private spawnProcess(): void {
    const args = [
      '--name', this.config.receiverName,
      '--output', 'stdout',
      // Output raw S16LE PCM at configured sample rate
      '-v', // verbose mode for metadata on stderr
    ];

    console.log(`[shairport] Starting: ${this.config.shairportPath} ${args.join(' ')}`);

    this.process = spawn(this.config.shairportPath, args, {
      stdio: ['ignore', 'pipe', 'pipe'],
    });

    if (this.process.stdout) {
      this.restartAttempts = 0;
      this.emit('audio', this.process.stdout);
    }

    if (this.process.stderr) {
      let buffer = '';
      this.process.stderr.on('data', (data: Buffer) => {
        buffer += data.toString();
        const lines = buffer.split('\n');
        buffer = lines.pop() || '';
        for (const line of lines) {
          this.parseMetadataLine(line);
        }
      });
    }

    this.process.on('close', (code, signal) => {
      console.log(`[shairport] Process exited (code=${code}, signal=${signal})`);
      this.process = null;
      this.emit('stopped');

      if (!this.restarting && code !== 0 && signal !== 'SIGTERM') {
        this.scheduleRestart();
      }
    });

    this.process.on('error', (err) => {
      console.error(`[shairport] Process error: ${err.message}`);
      this.emit('error', err);
    });

    this.emit('started');
  }

  private parseMetadataLine(line: string): void {
    // shairport-sync verbose output includes metadata hints
    const metadata: TrackMetadata = {};

    if (line.includes('Title:')) {
      metadata.title = line.split('Title:')[1]?.trim();
    } else if (line.includes('Artist:')) {
      metadata.artist = line.split('Artist:')[1]?.trim();
    } else if (line.includes('Album:')) {
      metadata.album = line.split('Album:')[1]?.trim();
    }

    if (metadata.title || metadata.artist || metadata.album) {
      this.emit('metadata', metadata);
    }
  }

  private scheduleRestart(): void {
    if (this.restartAttempts >= this.maxRestartAttempts) {
      console.error(`[shairport] Max restart attempts (${this.maxRestartAttempts}) reached. Giving up.`);
      this.emit('error', new Error('shairport-sync crashed too many times'));
      return;
    }

    this.restartAttempts++;
    const delay = this.restartDelay * Math.min(this.restartAttempts, 5);
    console.log(`[shairport] Restarting in ${delay}ms (attempt ${this.restartAttempts}/${this.maxRestartAttempts})`);

    setTimeout(() => {
      if (!this.process) {
        this.spawnProcess();
      }
    }, delay);
  }

  async stop(): Promise<void> {
    this.restarting = true;
    if (this.process) {
      this.process.kill('SIGTERM');
      await new Promise<void>((resolve) => {
        const timeout = setTimeout(() => {
          if (this.process) {
            this.process.kill('SIGKILL');
          }
          resolve();
        }, 5000);

        if (this.process) {
          this.process.on('close', () => {
            clearTimeout(timeout);
            resolve();
          });
        } else {
          clearTimeout(timeout);
          resolve();
        }
      });
      this.process = null;
    }
    this.restarting = false;
  }

  isRunning(): boolean {
    return this.process !== null;
  }
}
