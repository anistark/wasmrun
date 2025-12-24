import type {ReactNode} from 'react';
import {useState, useEffect} from 'react';
import Layout from '@theme/Layout';
import Heading from '@theme/Heading';
import styles from './community.module.css';

type Contributor = {
  login: string;
  avatar_url: string;
  html_url: string;
  contributions: number;
};

type Talk = {
  title: string;
  speaker: string;
  event: string;
  date: string;
  link?: string;
};

const maintainerUsernames = ['anistark', 'farhaanbukhsh'];

const maintainerRoles: Record<string, string> = {
  anistark: 'Core Maintainer',
  farhaanbukhsh: 'Core Maintainer',
};


const talks: Talk[] = [
];

function MaintainerCard({contributor, role}: {contributor: Contributor; role: string}) {
  return (
    <a
      href={contributor.html_url}
      target="_blank"
      rel="noopener noreferrer"
      className={styles.cardLink}
    >
      <div className={styles.card}>
        <div className={styles.cardContent}>
          <div className={styles.contributorHeader}>
            <img
              src={contributor.avatar_url}
              alt={contributor.login}
              className={styles.avatar}
            />
            <div>
              <Heading as="h3">{contributor.login}</Heading>
              <p className={styles.role}>{role}</p>
            </div>
          </div>
        </div>
      </div>
    </a>
  );
}

function ContributorCard({contributor}: {contributor: Contributor}) {
  return (
    <a
      href={contributor.html_url}
      target="_blank"
      rel="noopener noreferrer"
      className={styles.cardLink}
    >
      <div className={styles.card}>
        <div className={styles.cardContent}>
          <div className={styles.contributorHeader}>
            <img
              src={contributor.avatar_url}
              alt={contributor.login}
              className={styles.avatar}
            />
            <div>
              <Heading as="h4">{contributor.login}</Heading>
              <p className={styles.contributions}>{contributor.contributions} 🔥</p>
            </div>
          </div>
        </div>
      </div>
    </a>
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
  const [allContributors, setAllContributors] = useState<Contributor[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    fetch('https://api.github.com/repos/anistark/wasmrun/contributors')
      .then(response => response.json())
      .then(data => {
        setAllContributors(data);
        setLoading(false);
      })
      .catch(error => {
        console.error('Error fetching contributors:', error);
        setLoading(false);
      });
  }, []);

  const maintainers = allContributors.filter(c => maintainerUsernames.includes(c.login));
  const contributors = allContributors.filter(c => !maintainerUsernames.includes(c.login));

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
            {loading ? (
              <p className={styles.loading}>Loading maintainers...</p>
            ) : maintainers.length > 0 ? (
              <div className={styles.grid}>
                {maintainers.map((maintainer) => (
                  <MaintainerCard
                    key={maintainer.login}
                    contributor={maintainer}
                    role={maintainerRoles[maintainer.login] || 'Maintainer'}
                  />
                ))}
              </div>
            ) : (
              <p>No maintainers found.</p>
            )}
          </section>

          <section className={styles.section}>
            <Heading as="h2">Contributors</Heading>
            {loading ? (
              <p className={styles.loading}>Loading contributors...</p>
            ) : contributors.length > 0 ? (
              <div className={styles.grid}>
                {contributors.map((contributor) => (
                  <ContributorCard key={contributor.login} contributor={contributor} />
                ))}
              </div>
            ) : (
              <p>No contributors found.</p>
            )}
          </section>

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
