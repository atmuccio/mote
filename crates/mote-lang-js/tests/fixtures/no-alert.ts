declare function prompt2(message: string): string;
prompt2('x');
(window as any).alert('x'); // expect: no-alert
alert('typed' as string); // expect: no-alert
