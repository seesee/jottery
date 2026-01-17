#!/usr/bin/env node

/**
 * Test Coverage Report Generator
 *
 * Runs all tests across the Jottery project (web, server, TUI),
 * collects coverage data, stores results in SQLite for tracking,
 * and generates reports with charts.
 *
 * Usage:
 *   node scripts/test-report.js          # Run tests and generate report
 *   node scripts/test-report.js --quick  # Skip tests, show latest report
 *   node scripts/test-report.js --history # Show history and charts
 */

const { execSync, spawnSync } = require('child_process');
const path = require('path');
const fs = require('fs');
const Database = require('better-sqlite3');

// Optional: charts (may not be installed)
let asciichart;
let chalk;
try {
  asciichart = require('asciichart');
  chalk = require('chalk');
} catch {
  // Fallback if not installed
  chalk = {
    green: s => s,
    red: s => s,
    yellow: s => s,
    blue: s => s,
    cyan: s => s,
    gray: s => s,
    bold: s => s,
    dim: s => s,
  };
}

const ROOT_DIR = path.resolve(__dirname, '..');
const DB_PATH = path.join(ROOT_DIR, 'scripts', 'test-history.db');

// ============================================================================
// Database Setup
// ============================================================================

function initDatabase() {
  const db = new Database(DB_PATH);

  db.exec(`
    CREATE TABLE IF NOT EXISTS test_runs (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      timestamp TEXT NOT NULL,
      git_commit TEXT,
      git_branch TEXT,

      -- Web unit tests
      web_unit_total INTEGER,
      web_unit_passed INTEGER,
      web_unit_skipped INTEGER,
      web_unit_failed INTEGER,
      web_unit_line_coverage REAL,
      web_unit_branch_coverage REAL,
      web_unit_function_coverage REAL,

      -- Web E2E tests
      web_e2e_total INTEGER,
      web_e2e_passed INTEGER,
      web_e2e_failed INTEGER,

      -- Server tests
      server_total INTEGER,
      server_passed INTEGER,
      server_failed INTEGER,
      server_line_coverage REAL,
      server_function_coverage REAL,

      -- TUI tests
      tui_total INTEGER,
      tui_passed INTEGER,
      tui_failed INTEGER,
      tui_line_coverage REAL,
      tui_function_coverage REAL
    );

    CREATE TABLE IF NOT EXISTS test_files (
      id INTEGER PRIMARY KEY AUTOINCREMENT,
      run_id INTEGER NOT NULL,
      component TEXT NOT NULL,
      file_path TEXT NOT NULL,
      line_coverage REAL,
      function_coverage REAL,
      branch_coverage REAL,
      FOREIGN KEY (run_id) REFERENCES test_runs(id)
    );

    CREATE INDEX IF NOT EXISTS idx_test_runs_timestamp ON test_runs(timestamp);
    CREATE INDEX IF NOT EXISTS idx_test_files_run ON test_files(run_id);
  `);

  return db;
}

// ============================================================================
// Git Info
// ============================================================================

function getGitInfo() {
  try {
    const commit = execSync('git rev-parse --short HEAD', { cwd: ROOT_DIR, encoding: 'utf8' }).trim();
    const branch = execSync('git rev-parse --abbrev-ref HEAD', { cwd: ROOT_DIR, encoding: 'utf8' }).trim();
    return { commit, branch };
  } catch {
    return { commit: 'unknown', branch: 'unknown' };
  }
}

// ============================================================================
// Test Runners
// ============================================================================

function stripAnsi(str) {
  // Remove ANSI escape codes
  return str.replace(/\x1B\[[0-9;]*[a-zA-Z]/g, '');
}

function runWebUnitTests() {
  console.log(chalk.blue('\n📦 Running Web Unit Tests (Vitest)...\n'));

  try {
    const result = spawnSync('npm', ['test', '--', '--coverage', '--run'], {
      cwd: ROOT_DIR,
      encoding: 'utf8',
      timeout: 300000,
      maxBuffer: 50 * 1024 * 1024,
    });

    const output = stripAnsi(result.stdout + result.stderr);

    // Parse test counts - format: "386 passed | 7 skipped (393)"
    const testMatch = output.match(/(\d+)\s+passed\s*\|\s*(\d+)\s+skipped\s*\((\d+)\)/);
    const tests = {
      total: testMatch ? parseInt(testMatch[3]) : 0,
      passed: testMatch ? parseInt(testMatch[1]) : 0,
      skipped: testMatch ? parseInt(testMatch[2]) : 0,
      failed: 0,
    };

    // Parse coverage from output
    const coverageMatch = output.match(/All files\s+\|\s+([\d.]+)\s+\|\s+([\d.]+)\s+\|\s+([\d.]+)\s+\|\s+([\d.]+)/);
    const coverage = {
      statements: coverageMatch ? parseFloat(coverageMatch[1]) : 0,
      branches: coverageMatch ? parseFloat(coverageMatch[2]) : 0,
      functions: coverageMatch ? parseFloat(coverageMatch[3]) : 0,
      lines: coverageMatch ? parseFloat(coverageMatch[4]) : 0,
    };

    // Parse per-file coverage
    const filesCoverage = [];
    const fileRegex = /^\s*([\w./]+\.(?:ts|svelte))\s+\|\s+([\d.]+)\s+\|\s+([\d.]+)\s+\|\s+([\d.]+)\s+\|\s+([\d.]+)/gm;
    let match;
    while ((match = fileRegex.exec(output)) !== null) {
      if (match[1] !== 'All files') {
        filesCoverage.push({
          path: match[1],
          statements: parseFloat(match[2]),
          branches: parseFloat(match[3]),
          functions: parseFloat(match[4]),
          lines: parseFloat(match[5]),
        });
      }
    }

    return { tests, coverage, filesCoverage, success: result.status === 0 };
  } catch (err) {
    console.error(chalk.red('Error running web unit tests:'), err.message);
    return { tests: {}, coverage: {}, filesCoverage: [], success: false };
  }
}

function countE2ETests() {
  console.log(chalk.blue('\n🎭 Counting Web E2E Tests (Playwright)...\n'));

  try {
    const e2eDir = path.join(ROOT_DIR, 'e2e');
    const files = fs.readdirSync(e2eDir).filter(f => f.endsWith('.spec.ts'));

    let totalTests = 0;
    for (const file of files) {
      const content = fs.readFileSync(path.join(e2eDir, file), 'utf8');
      const matches = content.match(/test\(/g);
      totalTests += matches ? matches.length : 0;
    }

    return {
      total: totalTests,
      files: files.length,
      // E2E tests aren't run by default (slow), just count them
    };
  } catch (err) {
    console.error(chalk.red('Error counting E2E tests:'), err.message);
    return { total: 0, files: 0 };
  }
}

function runServerTests() {
  console.log(chalk.blue('\n🦀 Running Server Tests (Cargo + llvm-cov)...\n'));

  const serverDir = path.join(ROOT_DIR, 'server');

  try {
    const result = spawnSync('cargo', ['llvm-cov', '--summary-only'], {
      cwd: serverDir,
      encoding: 'utf8',
      timeout: 600000,
      maxBuffer: 50 * 1024 * 1024,
    });

    const output = result.stdout + result.stderr;

    // Count tests from output
    const testMatches = output.matchAll(/test result: ok\. (\d+) passed/g);
    let totalPassed = 0;
    for (const match of testMatches) {
      totalPassed += parseInt(match[1]);
    }

    // Parse coverage summary (TOTAL line)
    const coverageMatch = output.match(/TOTAL\s+\d+\s+\d+\s+([\d.]+)%\s+\d+\s+\d+\s+([\d.]+)%\s+\d+\s+\d+\s+([\d.]+)%/);
    const coverage = {
      regions: coverageMatch ? parseFloat(coverageMatch[1]) : 0,
      functions: coverageMatch ? parseFloat(coverageMatch[2]) : 0,
      lines: coverageMatch ? parseFloat(coverageMatch[3]) : 0,
    };

    // Parse per-file coverage
    const filesCoverage = [];
    const fileRegex = /^([\w/]+\.rs)\s+\d+\s+\d+\s+([\d.]+)%\s+\d+\s+\d+\s+([\d.]+)%\s+\d+\s+\d+\s+([\d.]+)%/gm;
    let match;
    while ((match = fileRegex.exec(output)) !== null) {
      filesCoverage.push({
        path: match[1],
        regions: parseFloat(match[2]),
        functions: parseFloat(match[3]),
        lines: parseFloat(match[4]),
      });
    }

    return {
      tests: { total: totalPassed, passed: totalPassed, failed: 0 },
      coverage,
      filesCoverage,
      success: result.status === 0,
    };
  } catch (err) {
    console.error(chalk.red('Error running server tests:'), err.message);
    return { tests: {}, coverage: {}, filesCoverage: [], success: false };
  }
}

function runTuiTests() {
  console.log(chalk.blue('\n🖥️  Running TUI Tests (Cargo + llvm-cov)...\n'));

  const tuiDir = path.join(ROOT_DIR, 'tui');

  try {
    const result = spawnSync('cargo', ['llvm-cov', '--summary-only'], {
      cwd: tuiDir,
      encoding: 'utf8',
      timeout: 600000,
      maxBuffer: 50 * 1024 * 1024,
    });

    const output = result.stdout + result.stderr;

    // Count tests from output
    const testMatches = output.matchAll(/test result: ok\. (\d+) passed/g);
    let totalPassed = 0;
    for (const match of testMatches) {
      totalPassed += parseInt(match[1]);
    }

    // Parse coverage summary
    const coverageMatch = output.match(/TOTAL\s+\d+\s+\d+\s+([\d.]+)%\s+\d+\s+\d+\s+([\d.]+)%\s+\d+\s+\d+\s+([\d.]+)%/);
    const coverage = {
      regions: coverageMatch ? parseFloat(coverageMatch[1]) : 0,
      functions: coverageMatch ? parseFloat(coverageMatch[2]) : 0,
      lines: coverageMatch ? parseFloat(coverageMatch[3]) : 0,
    };

    // Parse per-file coverage
    const filesCoverage = [];
    const fileRegex = /^([\w/]+\.rs)\s+\d+\s+\d+\s+([\d.]+)%\s+\d+\s+\d+\s+([\d.]+)%\s+\d+\s+\d+\s+([\d.]+)%/gm;
    let match;
    while ((match = fileRegex.exec(output)) !== null) {
      filesCoverage.push({
        path: match[1],
        regions: parseFloat(match[2]),
        functions: parseFloat(match[3]),
        lines: parseFloat(match[4]),
      });
    }

    return {
      tests: { total: totalPassed, passed: totalPassed, failed: 0 },
      coverage,
      filesCoverage,
      success: result.status === 0,
    };
  } catch (err) {
    console.error(chalk.red('Error running TUI tests:'), err.message);
    return { tests: {}, coverage: {}, filesCoverage: [], success: false };
  }
}

// ============================================================================
// Database Operations
// ============================================================================

function saveResults(db, results) {
  const { git, web, e2e, server, tui } = results;

  const stmt = db.prepare(`
    INSERT INTO test_runs (
      timestamp, git_commit, git_branch,
      web_unit_total, web_unit_passed, web_unit_skipped, web_unit_failed,
      web_unit_line_coverage, web_unit_branch_coverage, web_unit_function_coverage,
      web_e2e_total, web_e2e_passed, web_e2e_failed,
      server_total, server_passed, server_failed,
      server_line_coverage, server_function_coverage,
      tui_total, tui_passed, tui_failed,
      tui_line_coverage, tui_function_coverage
    ) VALUES (
      @timestamp, @git_commit, @git_branch,
      @web_unit_total, @web_unit_passed, @web_unit_skipped, @web_unit_failed,
      @web_unit_line_coverage, @web_unit_branch_coverage, @web_unit_function_coverage,
      @web_e2e_total, @web_e2e_passed, @web_e2e_failed,
      @server_total, @server_passed, @server_failed,
      @server_line_coverage, @server_function_coverage,
      @tui_total, @tui_passed, @tui_failed,
      @tui_line_coverage, @tui_function_coverage
    )
  `);

  const info = stmt.run({
    timestamp: new Date().toISOString(),
    git_commit: git.commit,
    git_branch: git.branch,
    web_unit_total: web.tests.total || 0,
    web_unit_passed: web.tests.passed || 0,
    web_unit_skipped: web.tests.skipped || 0,
    web_unit_failed: web.tests.failed || 0,
    web_unit_line_coverage: web.coverage.lines || 0,
    web_unit_branch_coverage: web.coverage.branches || 0,
    web_unit_function_coverage: web.coverage.functions || 0,
    web_e2e_total: e2e.total || 0,
    web_e2e_passed: null,
    web_e2e_failed: null,
    server_total: server.tests.total || 0,
    server_passed: server.tests.passed || 0,
    server_failed: server.tests.failed || 0,
    server_line_coverage: server.coverage.lines || 0,
    server_function_coverage: server.coverage.functions || 0,
    tui_total: tui.tests.total || 0,
    tui_passed: tui.tests.passed || 0,
    tui_failed: tui.tests.failed || 0,
    tui_line_coverage: tui.coverage.lines || 0,
    tui_function_coverage: tui.coverage.functions || 0,
  });

  const runId = info.lastInsertRowid;

  // Save per-file coverage
  const fileStmt = db.prepare(`
    INSERT INTO test_files (run_id, component, file_path, line_coverage, function_coverage, branch_coverage)
    VALUES (@run_id, @component, @file_path, @line_coverage, @function_coverage, @branch_coverage)
  `);

  for (const file of web.filesCoverage || []) {
    fileStmt.run({
      run_id: runId,
      component: 'web',
      file_path: file.path,
      line_coverage: file.lines,
      function_coverage: file.functions,
      branch_coverage: file.branches,
    });
  }

  for (const file of server.filesCoverage || []) {
    fileStmt.run({
      run_id: runId,
      component: 'server',
      file_path: file.path,
      line_coverage: file.lines,
      function_coverage: file.functions,
      branch_coverage: null,
    });
  }

  for (const file of tui.filesCoverage || []) {
    fileStmt.run({
      run_id: runId,
      component: 'tui',
      file_path: file.path,
      line_coverage: file.lines,
      function_coverage: file.functions,
      branch_coverage: null,
    });
  }

  return runId;
}

function getLatestRun(db) {
  return db.prepare('SELECT * FROM test_runs ORDER BY timestamp DESC LIMIT 1').get();
}

function getHistory(db, limit = 20) {
  return db.prepare('SELECT * FROM test_runs ORDER BY timestamp DESC LIMIT ?').all(limit);
}

function getRunComparison(db) {
  const runs = db.prepare('SELECT * FROM test_runs ORDER BY timestamp DESC LIMIT 2').all();
  if (runs.length < 2) return null;
  return { current: runs[0], previous: runs[1] };
}

// ============================================================================
// Report Generation
// ============================================================================

function formatCoverage(value, threshold = { good: 70, ok: 50 }) {
  if (value === null || value === undefined) return chalk.gray('N/A');
  const pct = value.toFixed(1) + '%';
  if (value >= threshold.good) return chalk.green(pct);
  if (value >= threshold.ok) return chalk.yellow(pct);
  return chalk.red(pct);
}

function formatDelta(current, previous) {
  if (previous === null || previous === undefined) return '';
  const delta = current - previous;
  if (Math.abs(delta) < 0.1) return chalk.gray(' (=)');
  const sign = delta > 0 ? '+' : '';
  const color = delta > 0 ? chalk.green : chalk.red;
  return color(` (${sign}${delta.toFixed(1)})`);
}

function generateReport(db, results) {
  const comparison = getRunComparison(db);
  const prev = comparison?.previous;

  const { git, web, e2e, server, tui } = results;

  const totalTests = (web.tests.total || 0) + (e2e.total || 0) +
                     (server.tests.total || 0) + (tui.tests.total || 0);

  console.log('\n' + '='.repeat(70));
  console.log(chalk.bold.cyan('                    JOTTERY TEST COVERAGE REPORT'));
  console.log('='.repeat(70));
  console.log(chalk.gray(`Generated: ${new Date().toISOString()}`));
  console.log(chalk.gray(`Git: ${git.branch}@${git.commit}`));
  console.log('='.repeat(70));

  // Summary Table
  console.log(chalk.bold('\n📊 SUMMARY\n'));
  console.log('┌─────────────────────┬─────────┬───────────────┬───────────────┐');
  console.log('│ Component           │  Tests  │ Line Coverage │ Func Coverage │');
  console.log('├─────────────────────┼─────────┼───────────────┼───────────────┤');

  const webTests = `${web.tests.passed || 0}/${web.tests.total || 0}`;
  const webLineDelta = prev ? formatDelta(web.coverage.lines, prev.web_unit_line_coverage) : '';
  console.log(`│ Web Unit (Vitest)   │ ${webTests.padStart(7)} │ ${formatCoverage(web.coverage.lines).padStart(13)}${webLineDelta.padEnd(15)} │ ${formatCoverage(web.coverage.functions).padStart(13)} │`);

  console.log(`│ Web E2E (Playwright)│ ${String(e2e.total || 0).padStart(7)} │ ${chalk.gray('    N/A').padStart(13)} │ ${chalk.gray('    N/A').padStart(13)} │`);

  const serverTests = `${server.tests.passed || 0}/${server.tests.total || 0}`;
  const serverLineDelta = prev ? formatDelta(server.coverage.lines, prev.server_line_coverage) : '';
  console.log(`│ Server (Rust/Axum)  │ ${serverTests.padStart(7)} │ ${formatCoverage(server.coverage.lines).padStart(13)}${serverLineDelta.padEnd(15)} │ ${formatCoverage(server.coverage.functions).padStart(13)} │`);

  const tuiTests = `${tui.tests.passed || 0}/${tui.tests.total || 0}`;
  const tuiLineDelta = prev ? formatDelta(tui.coverage.lines, prev.tui_line_coverage) : '';
  console.log(`│ TUI (Rust/Ratatui)  │ ${tuiTests.padStart(7)} │ ${formatCoverage(tui.coverage.lines).padStart(13)}${tuiLineDelta.padEnd(15)} │ ${formatCoverage(tui.coverage.functions).padStart(13)} │`);

  console.log('├─────────────────────┼─────────┼───────────────┼───────────────┤');
  console.log(`│ ${chalk.bold('TOTAL')}               │ ${chalk.bold(String(totalTests).padStart(7))} │               │               │`);
  console.log('└─────────────────────┴─────────┴───────────────┴───────────────┘');

  // Changes since last run
  if (prev) {
    const testDelta = totalTests - (prev.web_unit_total + prev.web_e2e_total + prev.server_total + prev.tui_total);
    if (testDelta !== 0) {
      const sign = testDelta > 0 ? '+' : '';
      const color = testDelta > 0 ? chalk.green : chalk.red;
      console.log(color(`\n${sign}${testDelta} tests since last run (${prev.git_commit})`));
    }
  }

  return totalTests;
}

function showHistory(db) {
  const history = getHistory(db, 15).reverse();

  if (history.length === 0) {
    console.log(chalk.yellow('\nNo test history available yet. Run tests first.\n'));
    return;
  }

  console.log('\n' + '='.repeat(70));
  console.log(chalk.bold.cyan('                    TEST HISTORY'));
  console.log('='.repeat(70));

  // Table
  console.log('\n┌────────────────────┬────────┬─────────┬─────────┬─────────┐');
  console.log('│ Date               │ Commit │ Web Cov │ Srv Cov │ TUI Cov │');
  console.log('├────────────────────┼────────┼─────────┼─────────┼─────────┤');

  for (const run of history) {
    const date = new Date(run.timestamp).toLocaleDateString('en-GB', {
      month: 'short', day: '2-digit', hour: '2-digit', minute: '2-digit'
    });
    console.log(`│ ${date.padEnd(18)} │ ${(run.git_commit || '?').padEnd(6)} │ ${formatCoverage(run.web_unit_line_coverage).padStart(7)} │ ${formatCoverage(run.server_line_coverage).padStart(7)} │ ${formatCoverage(run.tui_line_coverage).padStart(7)} │`);
  }

  console.log('└────────────────────┴────────┴─────────┴─────────┴─────────┘');

  // Charts (if enough data points)
  if (history.length >= 3 && asciichart) {
    console.log(chalk.bold('\n📈 COVERAGE TRENDS\n'));

    const webCov = history.map(r => r.web_unit_line_coverage || 0);
    const serverCov = history.map(r => r.server_line_coverage || 0);
    const tuiCov = history.map(r => r.tui_line_coverage || 0);

    console.log(chalk.cyan('Web Unit Coverage (Lines):'));
    console.log(asciichart.plot(webCov, { height: 8, colors: [asciichart.cyan] }));

    console.log(chalk.green('\nServer Coverage (Lines):'));
    console.log(asciichart.plot(serverCov, { height: 8, colors: [asciichart.green] }));

    console.log(chalk.yellow('\nTUI Coverage (Lines):'));
    console.log(asciichart.plot(tuiCov, { height: 8, colors: [asciichart.yellow] }));

    // Test count trend
    const testCounts = history.map(r =>
      (r.web_unit_total || 0) + (r.web_e2e_total || 0) + (r.server_total || 0) + (r.tui_total || 0)
    );

    console.log(chalk.magenta('\nTotal Test Count:'));
    console.log(asciichart.plot(testCounts, { height: 8, colors: [asciichart.magenta] }));
  } else if (history.length < 3) {
    console.log(chalk.gray('\n(Run tests a few more times to see coverage trend charts)\n'));
  }
}

// ============================================================================
// Main
// ============================================================================

async function main() {
  const args = process.argv.slice(2);
  const quickMode = args.includes('--quick');
  const historyMode = args.includes('--history');

  const db = initDatabase();

  if (historyMode) {
    showHistory(db);
    db.close();
    return;
  }

  if (quickMode) {
    const latest = getLatestRun(db);
    if (latest) {
      console.log(chalk.cyan('\n📋 Latest test run from database:\n'));
      console.log(`Timestamp: ${latest.timestamp}`);
      console.log(`Commit: ${latest.git_branch}@${latest.git_commit}`);
      console.log(`Web: ${latest.web_unit_passed}/${latest.web_unit_total} tests, ${latest.web_unit_line_coverage?.toFixed(1)}% coverage`);
      console.log(`E2E: ${latest.web_e2e_total} tests`);
      console.log(`Server: ${latest.server_passed}/${latest.server_total} tests, ${latest.server_line_coverage?.toFixed(1)}% coverage`);
      console.log(`TUI: ${latest.tui_passed}/${latest.tui_total} tests, ${latest.tui_line_coverage?.toFixed(1)}% coverage`);
    } else {
      console.log(chalk.yellow('No previous test runs found. Run without --quick first.'));
    }
    db.close();
    return;
  }

  console.log(chalk.bold.cyan('\n🧪 JOTTERY TEST SUITE\n'));
  console.log(chalk.gray('Running all tests and collecting coverage...\n'));

  const git = getGitInfo();
  const web = runWebUnitTests();
  const e2e = countE2ETests();
  const server = runServerTests();
  const tui = runTuiTests();

  const results = { git, web, e2e, server, tui };

  // Save to database
  const runId = saveResults(db, results);
  console.log(chalk.gray(`\nResults saved to database (run #${runId})`));

  // Generate report
  generateReport(db, results);

  // Show mini history
  const historyCount = db.prepare('SELECT COUNT(*) as count FROM test_runs').get().count;
  if (historyCount > 1) {
    console.log(chalk.gray(`\nRun with --history to see ${historyCount} historical runs and charts.`));
  }

  db.close();
}

main().catch(err => {
  console.error(chalk.red('Error:'), err);
  process.exit(1);
});
