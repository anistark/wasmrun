import React, { useEffect, useState } from 'react';
import ReactMarkdown from 'react-markdown';
import styles from './Changelog.module.css';

const CHANGELOG_URL = 'https://raw.githubusercontent.com/anistark/wasmrun/main/CHANGELOG.md';

export default function Changelog(): JSX.Element {
  const [content, setContent] = useState<string>('Loading changelog...');
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    fetch(CHANGELOG_URL)
      .then((response) => {
        if (!response.ok) {
          throw new Error(`Failed to fetch changelog: ${response.statusText}`);
        }
        return response.text();
      })
      .then((text) => {
        // Parse and clean up the changelog content
        let cleaned = text;

        // Remove the top "# Changelog" heading
        cleaned = cleaned.replace(/^# Changelog\s*\n/, '');

        // Remove "All notable changes..." line
        cleaned = cleaned.replace(/^All notable changes to.*?\n\n?/m, '');

        // Remove the "Keep a Changelog" format line and semver line
        cleaned = cleaned.replace(/^The format is based on.*?\n/m, '');
        cleaned = cleaned.replace(/^and this project adheres to.*?\n\n?/m, '');

        // Replace [Unreleased] with Upcoming
        cleaned = cleaned.replace(/## \[Unreleased\]/g, '## Upcoming');

        setContent(cleaned);
      })
      .catch((err) => {
        console.error('Error fetching changelog:', err);
        setError(err.message);
        setContent('Failed to load changelog. Please visit the [GitHub repository](https://github.com/anistark/wasmrun/blob/main/CHANGELOG.md).');
      });
  }, []);

  if (error) {
    return (
      <div className={styles.changelogError}>
        <p>⚠️ Failed to load changelog from GitHub.</p>
        <p>
          Please view it directly:{' '}
          <a href="https://github.com/anistark/wasmrun/blob/main/CHANGELOG.md" target="_blank" rel="noopener noreferrer">
            CHANGELOG.md on GitHub
          </a>
        </p>
      </div>
    );
  }

  return (
    <div className={styles.changelog}>
      <ReactMarkdown>{content}</ReactMarkdown>
    </div>
  );
}
