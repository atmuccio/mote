export const Button = () => {
  const variant = 'primary' as 'primary'; // expect: prefer-as-const
  return <button className={variant} />;
};
