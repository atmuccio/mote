// JSX compiled for another runtime (no React import, no React global): keys are not a concept.
import { TextChunk, PromptElement } from '@vscode/prompt-tsx';

export class Prompt extends PromptElement {
  render() {
    return <>{this.props.lines.map((line) => <TextChunk>{line}</TextChunk>)}</>;
  }
}
