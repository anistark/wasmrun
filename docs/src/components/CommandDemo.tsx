import React from 'react';
import CodeBlock from '@theme/CodeBlock';
import styles from './CommandDemo.module.css';

interface CommandDemoProps {
  command: string;
  language?: string;
  output?: string;
  showCopy?: boolean;
}

export default function CommandDemo({
  command,
  language = 'bash',
  output,
  showCopy = true,
}: CommandDemoProps): React.ReactElement {
  return (
    <div className={styles.commandDemo}>
      <div className={styles.commandSection}>
        <div className={styles.sectionLabel}>Command:</div>
        <CodeBlock language={language} showLineNumbers={false}>
          {command}
        </CodeBlock>
      </div>

      {output && (
        <div className={styles.outputSection}>
          <div className={styles.sectionLabel}>Output:</div>
          <CodeBlock language="text" showLineNumbers={false}>
            {output}
          </CodeBlock>
        </div>
      )}
    </div>
  );
}
