import type {ReactNode} from 'react';
import {useState} from 'react';
import clsx from 'clsx';
import Link from '@docusaurus/Link';
import useDocusaurusContext from '@docusaurus/useDocusaurusContext';
import Layout from '@theme/Layout';
import Heading from '@theme/Heading';

import styles from './index.module.css';

function InstallCommand() {
  const [copied, setCopied] = useState(false);
  const command = 'cargo install wasmrun';

  const copy = () => {
    navigator.clipboard.writeText(command).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    });
  };

  return (
    <button className={styles.installCommand} onClick={copy} type="button">
      <span className={styles.installPrompt}>$</span>
      <code>{command}</code>
      <span className={styles.installCopy} aria-live="polite">
        {copied ? (
          '✓ copied'
        ) : (
          <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" strokeWidth="2" aria-label="Copy to clipboard">
            <rect x="9" y="9" width="11" height="11" rx="2" />
            <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
          </svg>
        )}
      </span>
    </button>
  );
}

function TerminalMock() {
  return (
    <div className={styles.terminal}>
      <div className={styles.terminalBar}>
        <span className={styles.terminalDot} />
        <span className={styles.terminalDot} />
        <span className={styles.terminalDot} />
        <span className={styles.terminalTitle}>wasmrun</span>
      </div>
      <pre className={styles.terminalBody}>
        <span className={styles.tLine}>
          <span className={styles.tPrompt}>$ </span>wasmrun ./my-rust-project --watch
        </span>
        <span className={styles.tLine}>
          <span className={styles.tOk}>✓</span> Detected Rust project
        </span>
        <span className={styles.tLine}>
          <span className={styles.tOk}>✓</span> Compiled to WebAssembly in 1.2s
        </span>
        <span className={styles.tLine}>
          <span className={styles.tAccent}>▲</span> Serving on{' '}
          <span className={styles.tAccent}>http://localhost:8420</span> (live reload enabled)
        </span>
        <span className={styles.tCursor} />
      </pre>
    </div>
  );
}

function HomepageHeader() {
  return (
    <header className={styles.hero}>
      <div className={clsx('container', styles.heroInner)}>
        <p className={styles.heroBadge}>Open-source WebAssembly runtime</p>
        <Heading as="h1" className={styles.heroTitle}>
          Run WebAssembly,
          <br />
          <span className={styles.heroTitleAccent}>anywhere.</span>
        </Heading>
        <p className={styles.heroSubtitle}>
          Compile, serve, and execute WASM from Rust, Go, Python, C/C++ and
          AssemblyScript, with one zero-config CLI.
        </p>
        <InstallCommand />
        <div className={styles.heroButtons}>
          <Link className={clsx(styles.buttonPrimary)} to="/docs/intro">
            Get started
          </Link>
          <Link
            className={clsx(styles.buttonGhost)}
            href="https://github.com/anistark/wasmrun">
            GitHub ↗
          </Link>
        </div>
        <TerminalMock />
      </div>
    </header>
  );
}

type Mode = {
  title: string;
  command: string;
  description: string;
  to: string;
  icon: ReactNode;
};

const iconProps = {
  viewBox: '0 0 24 24',
  width: 26,
  height: 26,
  fill: 'none',
  stroke: 'currentColor',
  strokeWidth: 1.6,
  strokeLinecap: 'round',
  strokeLinejoin: 'round',
} as const;

const modes: Mode[] = [
  {
    title: 'Server Mode',
    command: 'wasmrun',
    description:
      'A built-in dev server with live reload and browser-based module inspection for full WASM projects.',
    to: '/docs/server',
    icon: (
      <svg {...iconProps}>
        <circle cx="12" cy="12" r="9" />
        <path d="M3 12h18M12 3a15 15 0 0 1 0 18M12 3a15 15 0 0 0 0 18" />
      </svg>
    ),
  },
  {
    title: 'Exec Mode',
    command: 'wasmrun exec',
    description:
      'Run .wasm files natively with the built-in interpreter and WASI support. No browser, no server.',
    to: '/docs/exec',
    icon: (
      <svg {...iconProps}>
        <path d="M4 17l6-5-6-5" />
        <path d="M12 19h8" />
      </svg>
    ),
  },
  {
    title: 'OS Mode',
    command: 'wasmrun os',
    description:
      'A sandboxed, browser-based execution environment with a virtual filesystem and network isolation.',
    to: '/docs/os',
    icon: (
      <svg {...iconProps}>
        <rect x="3" y="3" width="18" height="18" rx="3" />
        <path d="M3 9h18M9 21V9" />
      </svg>
    ),
  },
];

function ModeCards() {
  return (
    <section className={styles.modes}>
      <div className="container">
        <div className={styles.modeGrid}>
          {modes.map((mode) => (
            <Link key={mode.title} to={mode.to} className={styles.modeCard}>
              <span className={styles.modeIcon}>{mode.icon}</span>
              <Heading as="h3">{mode.title}</Heading>
              <code className={styles.modeCommand}>{mode.command}</code>
              <p>{mode.description}</p>
              <span className={styles.modeMore}>Learn more →</span>
            </Link>
          ))}
        </div>
      </div>
    </section>
  );
}

type Feature = {title: string; description: string; icon: ReactNode};

const features: Feature[] = [
  {
    title: 'Multi-language',
    description: 'Rust, Go, Python, C/C++ and AssemblyScript out of the box.',
    icon: (
      <svg {...iconProps}>
        <path d="M7 8l-4 4 4 4M17 8l4 4-4 4M14 4l-4 16" />
      </svg>
    ),
  },
  {
    title: 'Plugin architecture',
    description: 'Extensible system for language support and build tools.',
    icon: (
      <svg {...iconProps}>
        <path d="M9 3v4a2 2 0 0 1-2 2H3M15 3v4a2 2 0 0 0 2 2h4M9 21v-4a2 2 0 0 0-2-2H3M15 21v-4a2 2 0 0 1 2-2h4" />
      </svg>
    ),
  },
  {
    title: 'Live reload',
    description: 'File watching with automatic recompilation as you save.',
    icon: (
      <svg {...iconProps}>
        <path d="M21 12a9 9 0 1 1-2.64-6.36M21 3v6h-6" />
      </svg>
    ),
  },
  {
    title: 'Native interpreter',
    description: 'Built-in WASM interpreter with WASI syscall support.',
    icon: (
      <svg {...iconProps}>
        <rect x="4" y="4" width="16" height="16" rx="2" />
        <path d="M9 1v3M15 1v3M9 20v3M15 20v3M1 9h3M1 15h3M20 9h3M20 15h3" />
      </svg>
    ),
  },
  {
    title: 'Sandboxed',
    description: 'Isolated execution with per-session filesystems and limits.',
    icon: (
      <svg {...iconProps}>
        <path d="M12 3l8 4v5c0 5-3.5 8-8 9-4.5-1-8-4-8-9V7l8-4z" />
      </svg>
    ),
  },
  {
    title: 'Zero config',
    description: 'Auto-detects your project type. Sensible defaults everywhere.',
    icon: (
      <svg {...iconProps}>
        <path d="M13 2L4 14h6l-1 8 9-12h-6l1-8z" />
      </svg>
    ),
  },
];

function FeatureGrid() {
  return (
    <section className={styles.featuresSection}>
      <div className="container">
        <div className={styles.featureGrid}>
          {features.map((feature) => (
            <div key={feature.title} className={styles.featureItem}>
              <span className={styles.featureIcon}>{feature.icon}</span>
              <div>
                <Heading as="h4">{feature.title}</Heading>
                <p>{feature.description}</p>
              </div>
            </div>
          ))}
        </div>
      </div>
    </section>
  );
}

export default function Home(): ReactNode {
  const {siteConfig} = useDocusaurusContext();
  return (
    <Layout
      title={siteConfig.tagline}
      description="Wasmrun is an open-source WebAssembly runtime: compile, serve, and execute WASM from Rust, Go, Python, C/C++ and AssemblyScript with one zero-config CLI.">
      <HomepageHeader />
      <main>
        <ModeCards />
        <FeatureGrid />
      </main>
    </Layout>
  );
}
