#!/usr/bin/env node

/**
 * Dev Server Harness TUI
 *
 * A terminal UI for managing and monitoring development servers:
 * - Main web app (Vite on port 5173)
 * - Admin web app (Vite on port 5174)
 * - Sync server (Rust on port 3000)
 *
 * Usage: node scripts/dev-harness.js [options]
 *
 * Options:
 *   -k, --kill  Kill any processes on server ports before starting
 *
 * Keys:
 *   1/2/3  - Focus server panel
 *   r      - Restart focused server
 *   s      - Stop focused server
 *   k      - Kill process on focused server's port
 *   a      - Start all servers
 *   R      - Restart all servers
 *   S      - Stop all servers
 *   K      - Kill processes on all ports
 *   c      - Clear log of focused server
 *   q      - Quit
 *   Tab    - Cycle focus
 */

import { spawn, exec } from 'child_process';
import { createInterface } from 'readline';
import { writeFileSync, existsSync, mkdirSync } from 'fs';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';
import { promisify } from 'util';

const execAsync = promisify(exec);

const __dirname = dirname(fileURLToPath(import.meta.url));
const projectRoot = join(__dirname, '..');

// ANSI escape codes
const ESC = '\x1b';
const CSI = `${ESC}[`;

const colors = {
  reset: `${CSI}0m`,
  bold: `${CSI}1m`,
  dim: `${CSI}2m`,
  red: `${CSI}31m`,
  green: `${CSI}32m`,
  yellow: `${CSI}33m`,
  blue: `${CSI}34m`,
  magenta: `${CSI}35m`,
  cyan: `${CSI}36m`,
  white: `${CSI}37m`,
  bgRed: `${CSI}41m`,
  bgGreen: `${CSI}42m`,
  bgYellow: `${CSI}43m`,
  bgBlue: `${CSI}44m`,
};

const clear = () => process.stdout.write(`${CSI}2J${CSI}H`);
const moveTo = (row, col) => process.stdout.write(`${CSI}${row};${col}H`);
const hideCursor = () => process.stdout.write(`${CSI}?25l`);
const showCursor = () => process.stdout.write(`${CSI}?25h`);

// Server configurations
const servers = {
  web: {
    name: 'Web App',
    shortName: 'WEB',
    color: colors.cyan,
    command: 'npm',
    args: ['run', 'dev'],
    cwd: projectRoot,
    port: 5173,
    process: null,
    status: 'stopped',
    logs: [],
    stats: { lines: 0, errors: 0, warnings: 0, lastActivity: null },
  },
  admin: {
    name: 'Admin App',
    shortName: 'ADM',
    color: colors.magenta,
    command: 'npm',
    args: ['run', 'dev', '--', '--port', '5174'],
    cwd: join(projectRoot, 'admin'),
    port: 5174,
    process: null,
    status: 'stopped',
    logs: [],
    stats: { lines: 0, errors: 0, warnings: 0, lastActivity: null },
  },
  server: {
    name: 'Sync Server',
    shortName: 'SRV',
    color: colors.yellow,
    command: 'cargo',
    args: ['run', '--bin', 'jottery-server'],
    cwd: join(projectRoot, 'server'),
    port: 3000,
    process: null,
    status: 'stopped',
    logs: [],
    stats: { lines: 0, errors: 0, warnings: 0, lastActivity: null },
  },
};

const serverKeys = Object.keys(servers);
let focusedServer = 0;
let running = true;

/**
 * Kill any process listening on the specified port
 */
async function killPort(port, server) {
  try {
    const { stdout } = await execAsync(`lsof -ti:${port}`);
    const pids = stdout.trim().split('\n').filter(Boolean);

    if (pids.length > 0) {
      for (const pid of pids) {
        try {
          await execAsync(`kill -9 ${pid}`);
          if (server) {
            addLog(server, `[Harness] Killed process ${pid} on port ${port}`);
          }
        } catch (err) {
          // Process may have already exited
        }
      }
      return true;
    } else {
      if (server) {
        addLog(server, `[Harness] No process found on port ${port}`);
      }
    }
    return false;
  } catch (err) {
    // No process found on port (exit code 1 from lsof)
    if (server) {
      addLog(server, `[Harness] No process found on port ${port}`);
    }
    return false;
  }
}

/**
 * Kill process on a single server's port
 */
async function killServerPort(key) {
  const server = servers[key];
  addLog(server, `[Harness] Killing process on port ${server.port}...`);
  await killPort(server.port, server);

  // Wait a moment for port to be released
  await new Promise(resolve => setTimeout(resolve, 500));
  render();
}

/**
 * Kill all processes on all server ports
 */
async function killAllPorts() {
  for (const key of serverKeys) {
    const server = servers[key];
    addLog(server, `[Harness] Killing process on port ${server.port}...`);
    await killPort(server.port, server);
  }

  // Wait a moment for ports to be released
  await new Promise(resolve => setTimeout(resolve, 500));
  render();
}

// Maximum log lines to keep per server
const MAX_LOG_LINES = 500;

function addLog(server, line, isError = false) {
  const timestamp = new Date().toLocaleTimeString();
  const entry = { timestamp, line: line.toString(), isError };
  server.logs.push(entry);
  server.stats.lines++;
  server.stats.lastActivity = new Date();

  if (line.includes('error') || line.includes('Error') || line.includes('ERROR')) {
    server.stats.errors++;
  }
  if (line.includes('warning') || line.includes('Warning') || line.includes('WARN')) {
    server.stats.warnings++;
  }

  // Trim old logs
  if (server.logs.length > MAX_LOG_LINES) {
    server.logs = server.logs.slice(-MAX_LOG_LINES);
  }
}

function startServer(key) {
  const server = servers[key];

  if (server.process) {
    addLog(server, `[Harness] Server already running`);
    return;
  }

  if (!existsSync(server.cwd)) {
    addLog(server, `[Harness] Directory not found: ${server.cwd}`, true);
    server.status = 'error';
    return;
  }

  addLog(server, `[Harness] Starting ${server.name}...`);
  server.status = 'starting';

  server.process = spawn(server.command, server.args, {
    cwd: server.cwd,
    env: process.env,
    shell: true,
  });

  server.process.stdout.on('data', (data) => {
    const lines = data.toString().split('\n').filter(l => l.trim());
    lines.forEach(line => {
      addLog(server, line);
      // Detect when server is ready
      if (line.includes('ready in') || line.includes('Listening on') || line.includes('Local:')) {
        server.status = 'running';
      }
    });
    render();
  });

  server.process.stderr.on('data', (data) => {
    const lines = data.toString().split('\n').filter(l => l.trim());
    lines.forEach(line => addLog(server, line, true));
    render();
  });

  server.process.on('close', (code) => {
    addLog(server, `[Harness] Process exited with code ${code}`);
    server.status = code === 0 ? 'stopped' : 'error';
    server.process = null;
    render();
  });

  server.process.on('error', (err) => {
    addLog(server, `[Harness] Error: ${err.message}`, true);
    server.status = 'error';
    server.process = null;
    render();
  });
}

function stopServer(key) {
  const server = servers[key];

  if (!server.process) {
    addLog(server, `[Harness] Server not running`);
    return;
  }

  addLog(server, `[Harness] Stopping ${server.name}...`);
  server.status = 'stopping';

  // Kill process group
  try {
    process.kill(-server.process.pid, 'SIGTERM');
  } catch {
    server.process.kill('SIGTERM');
  }

  // Force kill after timeout
  setTimeout(() => {
    if (server.process) {
      try {
        process.kill(-server.process.pid, 'SIGKILL');
      } catch {
        server.process.kill('SIGKILL');
      }
    }
  }, 3000);
}

function restartServer(key) {
  const server = servers[key];

  if (server.process) {
    addLog(server, `[Harness] Restarting ${server.name}...`);
    server.status = 'restarting';

    const onClose = () => {
      server.process = null;
      setTimeout(() => startServer(key), 500);
    };

    server.process.once('close', onClose);
    stopServer(key);
  } else {
    startServer(key);
  }
}

function getStatusIcon(status) {
  switch (status) {
    case 'running': return `${colors.green}●${colors.reset}`;
    case 'stopped': return `${colors.dim}○${colors.reset}`;
    case 'starting': return `${colors.yellow}◐${colors.reset}`;
    case 'stopping': return `${colors.yellow}◑${colors.reset}`;
    case 'restarting': return `${colors.yellow}◒${colors.reset}`;
    case 'error': return `${colors.red}✗${colors.reset}`;
    default: return `${colors.dim}?${colors.reset}`;
  }
}

function formatDuration(ms) {
  if (!ms) return '--';
  const seconds = Math.floor(ms / 1000);
  const minutes = Math.floor(seconds / 60);
  const hours = Math.floor(minutes / 60);

  if (hours > 0) return `${hours}h ${minutes % 60}m`;
  if (minutes > 0) return `${minutes}m ${seconds % 60}s`;
  return `${seconds}s`;
}

function render() {
  if (!running) return;

  const { columns, rows } = process.stdout;
  const panelWidth = Math.floor((columns - 4) / 3);
  const logHeight = rows - 8;

  clear();

  // Header
  moveTo(1, 1);
  process.stdout.write(
    `${colors.bold}${colors.blue}╔${'═'.repeat(columns - 2)}╗${colors.reset}`
  );
  moveTo(2, 1);
  process.stdout.write(
    `${colors.blue}║${colors.reset} ${colors.bold}Jottery Dev Harness${colors.reset}` +
    `${' '.repeat(columns - 24)}` +
    `${colors.blue}║${colors.reset}`
  );
  moveTo(3, 1);
  process.stdout.write(
    `${colors.blue}╠${'═'.repeat(columns - 2)}╣${colors.reset}`
  );

  // Server panels
  serverKeys.forEach((key, idx) => {
    const server = servers[key];
    const x = 2 + (idx * (panelWidth + 1));
    const isFocused = idx === focusedServer;
    const borderColor = isFocused ? server.color : colors.dim;

    // Panel header
    moveTo(4, x);
    const status = getStatusIcon(server.status);
    const headerText = ` [${idx + 1}] ${server.name} ${status} `;
    const padding = panelWidth - headerText.length + 6; // Account for ANSI codes
    process.stdout.write(
      `${borderColor}┌${'─'.repeat(Math.max(0, padding/2))}${colors.reset}` +
      `${isFocused ? colors.bold : ''}${headerText}${colors.reset}` +
      `${borderColor}${'─'.repeat(Math.max(0, padding/2))}┐${colors.reset}`
    );

    // Stats line
    moveTo(5, x);
    const statsLine = ` Port:${server.port} E:${server.stats.errors} W:${server.stats.warnings}`;
    process.stdout.write(
      `${borderColor}│${colors.reset}` +
      `${colors.dim}${statsLine.padEnd(panelWidth - 2)}${colors.reset}` +
      `${borderColor}│${colors.reset}`
    );

    // Log lines
    const visibleLogs = server.logs.slice(-(logHeight - 2));
    for (let i = 0; i < logHeight - 2; i++) {
      moveTo(6 + i, x);
      const log = visibleLogs[i];
      let line = '';
      if (log) {
        line = log.line.substring(0, panelWidth - 4);
        if (log.isError) {
          line = `${colors.red}${line}${colors.reset}`;
        }
      }
      // Calculate visible length (without ANSI codes)
      const visibleLen = line.replace(/\x1b\[[0-9;]*m/g, '').length;
      const padLen = Math.max(0, panelWidth - 2 - visibleLen);
      process.stdout.write(
        `${borderColor}│${colors.reset} ${line}${' '.repeat(padLen)}${borderColor}│${colors.reset}`
      );
    }

    // Panel footer
    moveTo(5 + logHeight - 1, x);
    process.stdout.write(
      `${borderColor}└${'─'.repeat(panelWidth - 2)}┘${colors.reset}`
    );
  });

  // Footer with help
  moveTo(rows - 2, 1);
  process.stdout.write(
    `${colors.blue}╠${'═'.repeat(columns - 2)}╣${colors.reset}`
  );
  moveTo(rows - 1, 1);
  const helpText = ' [1-3] Focus | [r]estart | [s]top | [k]ill port | [a]ll start | [R]estart all | [S]top all | [K]ill all ports | [c]lear | [q]uit ';
  process.stdout.write(
    `${colors.blue}║${colors.reset}${colors.dim}${helpText.padEnd(columns - 3)}${colors.reset}${colors.blue}║${colors.reset}`
  );
  moveTo(rows, 1);
  process.stdout.write(
    `${colors.blue}╚${'═'.repeat(columns - 2)}╝${colors.reset}`
  );
}

function handleKey(key) {
  const currentKey = serverKeys[focusedServer];

  switch (key) {
    case '1':
    case '2':
    case '3':
      focusedServer = parseInt(key) - 1;
      break;
    case '\t': // Tab
      focusedServer = (focusedServer + 1) % serverKeys.length;
      break;
    case 'r':
      restartServer(currentKey);
      break;
    case 's':
      stopServer(currentKey);
      break;
    case 'a':
      serverKeys.forEach(k => startServer(k));
      break;
    case 'R':
      serverKeys.forEach(k => restartServer(k));
      break;
    case 'S':
      serverKeys.forEach(k => stopServer(k));
      break;
    case 'k':
      // Kill process on focused server's port
      killServerPort(currentKey);
      break;
    case 'K':
      // Kill processes on all server ports
      killAllPorts();
      break;
    case 'c':
      servers[currentKey].logs = [];
      servers[currentKey].stats = { lines: 0, errors: 0, warnings: 0, lastActivity: null };
      break;
    case 'q':
    case '\x03': // Ctrl+C
      cleanup();
      break;
  }

  render();
}

function cleanup() {
  running = false;
  showCursor();

  // Stop all servers
  serverKeys.forEach(key => {
    const server = servers[key];
    if (server.process) {
      try {
        process.kill(-server.process.pid, 'SIGTERM');
      } catch {
        server.process.kill('SIGTERM');
      }
    }
  });

  clear();
  console.log('Dev harness stopped. Goodbye!');

  // Give servers time to stop gracefully
  setTimeout(() => process.exit(0), 1000);
}

// Main
async function main() {
  // Parse command-line arguments
  const args = process.argv.slice(2);
  const shouldKillPorts = args.includes('-k') || args.includes('--kill');

  // Check if admin directory exists
  if (!existsSync(join(projectRoot, 'admin'))) {
    console.log(`${colors.yellow}Warning: admin directory not found${colors.reset}`);
  }

  // Kill existing processes on ports if requested
  if (shouldKillPorts) {
    console.log('Killing existing processes on server ports...');
    for (const key of serverKeys) {
      const server = servers[key];
      await killPort(server.port, null);
    }
    console.log('Ports cleared. Starting dev harness...\n');
    await new Promise(resolve => setTimeout(resolve, 500));
  }

  // Setup terminal
  if (process.stdin.isTTY) {
    process.stdin.setRawMode(true);
  }
  process.stdin.resume();
  process.stdin.setEncoding('utf8');

  hideCursor();

  // Handle input
  process.stdin.on('data', handleKey);

  // Handle resize
  process.stdout.on('resize', render);

  // Handle exit signals
  process.on('SIGINT', cleanup);
  process.on('SIGTERM', cleanup);

  // Initial render
  render();

  // Auto-start all servers
  serverKeys.forEach(k => startServer(k));

  // Periodic render for updates
  setInterval(() => {
    if (running) render();
  }, 1000);
}

main().catch(err => {
  console.error('Error:', err);
  cleanup();
});
