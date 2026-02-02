import { describe, it, expect } from 'vitest';
import { cn } from '$lib/utils';

describe('cn utility', () => {
  it('merges class names', () => {
    const result = cn('foo', 'bar');
    expect(result).toBe('foo bar');
  });

  it('handles conditional classes', () => {
    const isActive = true;
    const isDisabled = false;

    const result = cn('base', isActive && 'active', isDisabled && 'disabled');
    expect(result).toBe('base active');
  });

  it('handles undefined and null', () => {
    const result = cn('foo', undefined, null, 'bar');
    expect(result).toBe('foo bar');
  });

  it('merges tailwind classes correctly', () => {
    // tailwind-merge should handle conflicting utilities
    const result = cn('p-4', 'p-2');
    expect(result).toBe('p-2');
  });

  it('merges responsive classes correctly', () => {
    const result = cn('md:p-4', 'md:p-2');
    expect(result).toBe('md:p-2');
  });

  it('handles arrays', () => {
    const result = cn(['foo', 'bar'], 'baz');
    expect(result).toBe('foo bar baz');
  });

  it('handles objects', () => {
    const result = cn({
      foo: true,
      bar: false,
      baz: true,
    });
    expect(result).toBe('foo baz');
  });

  it('handles complex combinations', () => {
    const variant: string = 'primary';
    const size: string = 'lg';
    const disabled = false;

    const result = cn(
      'btn',
      {
        'btn-primary': variant === 'primary',
        'btn-secondary': variant === 'secondary',
        'btn-lg': size === 'lg',
        'btn-sm': size === 'sm',
        'btn-disabled': disabled,
      },
      'custom-class'
    );

    expect(result).toBe('btn btn-primary btn-lg custom-class');
  });

  it('handles empty input', () => {
    const result = cn();
    expect(result).toBe('');
  });

  it('handles bg-color conflicts', () => {
    const result = cn('bg-red-500', 'bg-blue-500');
    expect(result).toBe('bg-blue-500');
  });

  it('preserves different utility types', () => {
    const result = cn('bg-red-500', 'text-white', 'p-4');
    expect(result).toBe('bg-red-500 text-white p-4');
  });
});
