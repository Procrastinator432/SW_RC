//! Original finite-priority sort (103d49d0), including its tie permutations.
pub fn order(words: &[u32]) -> Result<Vec<usize>, String> {
    if words.len() > 4096
        || words
            .iter()
            .any(|&w| !f32::from_bits(w).is_finite() || f32::from_bits(w).abs() > 1e6)
    {
        return Err("Invalid audio priority list".into());
    }
    let mut a = (0..words.len()).collect::<Vec<_>>();
    if a.len() < 2 {
        return Ok(a);
    }
    let p = words.iter().map(|&w| f32::from_bits(w)).collect::<Vec<_>>();
    let mut stack = vec![(0, a.len() - 1)];
    while let Some((mut lo, mut hi)) = stack.pop() {
        loop {
            let len = hi - lo + 1;
            if len <= 8 {
                while lo < hi {
                    let mut minimum = lo;
                    for j in lo + 1..=hi {
                        if p[a[minimum]] - p[a[j]] >= 0. {
                            minimum = j;
                        }
                    }
                    a.swap(minimum, hi);
                    hi -= 1;
                }
                break;
            }
            a.swap(lo, lo + len / 2);
            let mut left = lo;
            let mut right = hi + 1;
            loop {
                left += 1;
                while left <= hi && p[a[lo]] - p[a[left]] < 0. {
                    left += 1;
                }
                right -= 1;
                while right > lo && p[a[lo]] - p[a[right]] >= 0. {
                    right -= 1;
                }
                if left <= right {
                    a.swap(left, right);
                } else {
                    break;
                }
            }
            a.swap(lo, right);
            if right as isize - lo as isize - 1 < hi as isize - left as isize {
                if left < hi {
                    stack.push((left, hi));
                }
                if right <= lo + 1 {
                    break;
                }
                hi = right - 1;
            } else {
                if lo + 1 < right {
                    stack.push((lo, right - 1));
                }
                if hi <= left {
                    break;
                }
                lo = left;
            }
        }
    }
    Ok(a)
}
/// CVTTSS2SI truncation of the first priority below the physical channel budget.
pub fn cutoff(sorted_words: &[u32], channels: usize) -> Result<i32, String> {
    if channels > 4096
        || sorted_words.len() > 4096
        || sorted_words
            .iter()
            .any(|&w| !f32::from_bits(w).is_finite() || f32::from_bits(w).abs() > 1e6)
    {
        return Err("Invalid audio cutoff input".into());
    }
    Ok(if sorted_words.len() <= channels {
        0
    } else {
        f32::from_bits(sorted_words[channels]) as i32
    })
}
