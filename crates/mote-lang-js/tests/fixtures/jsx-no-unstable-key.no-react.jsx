import { h } from 'vue';

export const view = (items) => items.map((item) => <li key={Math.random()}>{item}</li>);
