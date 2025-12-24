import React from 'react';
import Tabs from '@theme/Tabs';
import TabItem from '@theme/TabItem';
import CodeBlock from '@theme/CodeBlock';

interface InstallTabsProps {
  defaultValue?: 'cargo' | 'deb' | 'rpm' | 'source';
}

export default function InstallTabs({ defaultValue = 'cargo' }: InstallTabsProps): JSX.Element {
  return (
    <Tabs defaultValue={defaultValue} groupId="installation-method">
      <TabItem value="cargo" label="Cargo (Recommended)">
        <p>Install via Cargo, Rust's package manager:</p>
        <CodeBlock language="bash">
          cargo install wasmrun
        </CodeBlock>
        <p><strong>Requirements:</strong></p>
        <ul>
          <li>Rust 1.70 or higher</li>
          <li>Cargo (comes with Rust)</li>
        </ul>
        <p>If you don't have Rust installed, get it from <a href="https://rustup.rs/">rustup.rs</a>.</p>
      </TabItem>

      <TabItem value="deb" label="DEB Package">
        <p>For Debian-based Linux distributions (Ubuntu, Debian, Pop!_OS, Linux Mint):</p>
        <ol>
          <li>Download the latest <code>.deb</code> file from <a href="https://github.com/anistark/wasmrun/releases">GitHub Releases</a></li>
          <li>Install the package:</li>
        </ol>
        <CodeBlock language="bash">
{`# Install the downloaded DEB package
sudo apt install ./wasmrun_*.deb

# If there are dependency issues, fix them
sudo apt install -f`}
        </CodeBlock>
        <p><strong>Supported distributions:</strong></p>
        <ul>
          <li>Ubuntu 20.04+</li>
          <li>Debian 11+</li>
          <li>Pop!_OS 20.04+</li>
          <li>Linux Mint 20+</li>
        </ul>
      </TabItem>

      <TabItem value="rpm" label="RPM Package">
        <p>For Red Hat-based Linux distributions (Fedora, RHEL, CentOS):</p>
        <ol>
          <li>Download the latest <code>.rpm</code> file from <a href="https://github.com/anistark/wasmrun/releases">GitHub Releases</a></li>
          <li>Install the package:</li>
        </ol>
        <CodeBlock language="bash">
{`# Install using rpm
sudo rpm -i wasmrun-*.rpm

# Or using dnf (Fedora/RHEL 8+)
sudo dnf install ./wasmrun-*.rpm

# Or using yum (older versions)
sudo yum install ./wasmrun-*.rpm`}
        </CodeBlock>
        <p><strong>Supported distributions:</strong></p>
        <ul>
          <li>Fedora 35+</li>
          <li>RHEL 8+</li>
          <li>CentOS Stream 8+</li>
          <li>Rocky Linux 8+</li>
        </ul>
      </TabItem>

      <TabItem value="source" label="From Source">
        <p>Build from source for the latest development version:</p>
        <CodeBlock language="bash">
{`# Clone the repository
git clone https://github.com/anistark/wasmrun.git
cd wasmrun

# Install from source
cargo install --path .`}
        </CodeBlock>
        <p><strong>Build requirements:</strong></p>
        <ul>
          <li>Rust 1.70 or higher</li>
          <li>Git</li>
        </ul>
      </TabItem>
    </Tabs>
  );
}
