import React from 'react';
import styles from './PluginCard.module.css';
import CodeBlock from '@theme/CodeBlock';

interface PluginCardProps {
  name: string;
  language: string;
  type: 'builtin' | 'external';
  installCmd?: string;
  description: string;
  requirements?: string[];
}

export default function PluginCard({
  name,
  language,
  type,
  installCmd,
  description,
  requirements = [],
}: PluginCardProps): React.ReactElement {
  const badge = type === 'builtin' ? '✓ Built-in' : '📦 External';
  const badgeClass = type === 'builtin' ? styles.builtinBadge : styles.externalBadge;

  return (
    <div className={styles.pluginCard}>
      <div className={styles.pluginHeader}>
        <h3 className={styles.pluginName}>{language}</h3>
        <span className={`${styles.badge} ${badgeClass}`}>{badge}</span>
      </div>

      <p className={styles.pluginDescription}>{description}</p>

      {type === 'external' && installCmd && (
        <div className={styles.installSection}>
          <strong>Installation:</strong>
          <CodeBlock language="bash">{installCmd}</CodeBlock>
        </div>
      )}

      {requirements.length > 0 && (
        <div className={styles.requirements}>
          <strong>Requirements:</strong>
          <ul>
            {requirements.map((req, index) => (
              <li key={index}>{req}</li>
            ))}
          </ul>
        </div>
      )}

      <div className={styles.pluginFooter}>
        <code>{name}</code>
      </div>
    </div>
  );
}
