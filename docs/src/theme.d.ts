/// <reference types="@docusaurus/module-type-aliases" />
/// <reference types="react" />

declare module '@theme/Layout' {
  import type { ReactNode } from 'react';
  export interface Props {
    readonly children?: ReactNode;
    readonly title?: string;
    readonly description?: string;
    readonly noFooter?: boolean;
    readonly wrapperClassName?: string;
  }
  export default function Layout(props: Props): ReactNode;
}

declare module '@theme/CodeBlock' {
  import type { ReactNode } from 'react';
  export interface Props {
    readonly children?: ReactNode;
    readonly className?: string;
    readonly language?: string;
    readonly title?: string;
    readonly showLineNumbers?: boolean;
  }
  export default function CodeBlock(props: Props): ReactNode;
}

declare module '@theme/Heading' {
  import type { ComponentProps, ReactNode } from 'react';
  export interface Props extends ComponentProps<'h1'> {
    readonly as?: 'h1' | 'h2' | 'h3' | 'h4' | 'h5' | 'h6';
  }
  export default function Heading(props: Props): ReactNode;
}

declare module '@theme/Tabs' {
  import type { ReactNode } from 'react';
  export interface Props {
    readonly children?: ReactNode;
    readonly defaultValue?: string;
    readonly values?: readonly { label: string; value: string }[];
    readonly groupId?: string;
    readonly className?: string;
    readonly lazy?: boolean;
  }
  export default function Tabs(props: Props): ReactNode;
}

declare module '@theme/TabItem' {
  import type { ReactNode } from 'react';
  export interface Props {
    readonly children?: ReactNode;
    readonly value: string;
    readonly label?: string;
    readonly default?: boolean;
    readonly className?: string;
    readonly attributes?: Record<string, unknown>;
  }
  export default function TabItem(props: Props): ReactNode;
}
