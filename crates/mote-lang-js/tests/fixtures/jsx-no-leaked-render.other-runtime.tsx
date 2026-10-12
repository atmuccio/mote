// A JSX file for another renderer (a prompt builder): nothing is reported without React evidence.
import { PromptElement, TextChunk } from 'some-prompt-library';

export class Prompt extends PromptElement {
  render(lines: string[]) {
    return (
      <>
        {lines.length && <TextChunk>{lines.join('\n')}</TextChunk>}
      </>
    );
  }
}
