export const enum Size { S, M } // expect: const-enum
export const View = () => <div>{Size.S}</div>;
