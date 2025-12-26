import type {ReactNode} from 'react';
import clsx from 'clsx';
import Heading from '@theme/Heading';
import styles from './styles.module.css';

type FeatureItem = {
  title: string;
  Svg?: React.ComponentType<React.ComponentProps<'svg'>>;
  image?: string;
  description: ReactNode;
};

const FeatureList: FeatureItem[] = [
  {
    title: 'Built with Rust',
    image: require('@site/static/img/ills/wasm-ferris.png').default,
    description: (
      <>
        Leveraging Rust&apos;s performance and safety guarantees to provide a blazing-fast
        development server. Memory-safe and reliable WebAssembly execution.
      </>
    ),
  },
  {
    title: 'Fullstack WASM',
    image: require('@site/static/img/ills/wasm-card-back.png').default,
    description: (
      <>
        Build complete applications with WebAssembly. Support for multiple languages including
        Rust, Go, Python, ASC and C/C++. Run anywhere with true cross-platform compatibility.
      </>
    ),
  },
  {
    title: 'Sandboxed & Secure',
    image: require('@site/static/img/ills/wasm-sandbox.png').default,
    description: (
      <>
        Isolated execution environments with per-process network namespaces.
        WASI socket API support ensures your applications run securely without compromising functionality.
      </>
    ),
  },
];

function Feature({title, Svg, image, description}: FeatureItem) {
  return (
    <div className={clsx('col col--4')}>
      <div className="text--center">
        {Svg && <Svg className={styles.featureSvg} role="img" />}
        {image && <img src={image} className={styles.featureSvg} alt={title} />}
      </div>
      <div className="text--center padding-horiz--md">
        <Heading as="h3">{title}</Heading>
        <p>{description}</p>
      </div>
    </div>
  );
}

export default function HomepageFeatures(): ReactNode {
  return (
    <section className={styles.features}>
      <div className="container">
        <div className="row">
          {FeatureList.map((props, idx) => (
            <Feature key={idx} {...props} />
          ))}
        </div>
      </div>
    </section>
  );
}
