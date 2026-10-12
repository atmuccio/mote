// Not React: a JSX prompt library whose helpers happen to be called use….
import { PromptElement } from '@vscode/prompt-tsx';

export function Section(props: { on: boolean }) {
  if (props.on) {
    useTokens(props);
  }
  return <div />;
}
declare function useTokens(p: unknown): void;
