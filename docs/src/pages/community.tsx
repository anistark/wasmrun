import type {ReactNode} from 'react';
import Layout from '@theme/Layout';
import Heading from '@theme/Heading';
import styles from './community.module.css';

type Maintainer = {
  name: string;
  role: string;
  github: string;
  avatar?: string;
};

type Contributor = {
  name: string;
  github: string;
  contributions: number;
};

type Talk = {
  title: string;
  speaker: string;
  event: string;
  date: string;
  link?: string;
};

const maintainers: Maintainer[] = [
  {
    name: 'Core Team',
    role: 'Lead Maintainer',
    github: 'https://github.com/anistark',
  },
];

const topContributors: Contributor[] = [
];

const talks: Talk[] = [
];

function MaintainerCard({maintainer}: {maintainer: Maintainer}) {
  return (
    <div className={styles.card}>
      <div className={styles.cardContent}>
        <Heading as="h3">{maintainer.name}</Heading>
        <p className={styles.role}>{maintainer.role}</p>
        <a href={maintainer.github} target="_blank" rel="noopener noreferrer" className={styles.link}>
          GitHub Profile
        </a>
      </div>
    </div>
  );
}

function ContributorCard({contributor}: {contributor: Contributor}) {
  return (
    <div className={styles.card}>
      <div className={styles.cardContent}>
        <Heading as="h4">{contributor.name}</Heading>
        <p className={styles.contributions}>{contributor.contributions} contributions</p>
        <a href={contributor.github} target="_blank" rel="noopener noreferrer" className={styles.link}>
          GitHub
        </a>
      </div>
    </div>
  );
}

function TalkCard({talk}: {talk: Talk}) {
  return (
    <div className={styles.card}>
      <div className={styles.cardContent}>
        <Heading as="h4">{talk.title}</Heading>
        <p className={styles.speaker}>By {talk.speaker}</p>
        <p className={styles.event}>{talk.event} • {talk.date}</p>
        {talk.link && (
          <a href={talk.link} target="_blank" rel="noopener noreferrer" className={styles.link}>
            Watch Talk
          </a>
        )}
      </div>
    </div>
  );
}

export default function Community(): ReactNode {
  return (
    <Layout
      title="Community"
      description="Meet the Wasmrun community - maintainers, contributors, and talks">
      <main className={styles.communityPage}>
        <div className="container">
          <div className={styles.header}>
            <Heading as="h1">Community</Heading>
            <p className={styles.subtitle}>
              Join the Wasmrun community and help us build the future of WebAssembly development
            </p>
          </div>

          <section className={styles.section}>
            <Heading as="h2">Maintainers</Heading>
            <div className={styles.grid}>
              {maintainers.map((maintainer, idx) => (
                <MaintainerCard key={idx} maintainer={maintainer} />
              ))}
            </div>
          </section>

          {topContributors.length > 0 && (
            <section className={styles.section}>
              <Heading as="h2">Top Contributors</Heading>
              <div className={styles.grid}>
                {topContributors.map((contributor, idx) => (
                  <ContributorCard key={idx} contributor={contributor} />
                ))}
              </div>
            </section>
          )}

          {talks.length > 0 && (
            <section className={styles.section}>
              <Heading as="h2">Talks & Presentations</Heading>
              <div className={styles.grid}>
                {talks.map((talk, idx) => (
                  <TalkCard key={idx} talk={talk} />
                ))}
              </div>
            </section>
          )}

          <section className={styles.section}>
            <Heading as="h2">Get Involved</Heading>
            <div className={styles.getInvolved}>
              <p>
                We welcome contributions from everyone! Here are some ways you can get involved:
              </p>
              <ul>
                <li>🐛 Report bugs and request features on <a href="https://github.com/anistark/wasmrun/issues">GitHub Issues</a></li>
                <li>💬 Join discussions on <a href="https://github.com/anistark/wasmrun/discussions">GitHub Discussions</a></li>
                <li>🔧 Submit pull requests to improve the codebase</li>
                <li>📖 Help improve documentation</li>
                <li>⭐ Star the project on <a href="https://github.com/anistark/wasmrun">GitHub</a></li>
              </ul>
            </div>
          </section>
        </div>
      </main>
    </Layout>
  );
}
