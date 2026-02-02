import Root from './alert.svelte';
import Description from './alert-description.svelte';
import Title from './alert-title.svelte';

export type AlertVariant = 'default' | 'destructive';

export {
  Root,
  Root as Alert,
  Description,
  Description as AlertDescription,
  Title,
  Title as AlertTitle,
};
