//! Bounded texture-stage walk and post-resolver writes from D3DDrv 1000dd24..de98.
//! Resource resolution/filter/address bits remain the host resolver's responsibility.
pub type Stage = [u32; 28];
pub type Resolver<'a, T> = dyn FnMut(usize, &T) -> Result<Option<Stage>, String> + 'a;
pub fn configure(stage: &mut Stage, index: usize, transform: [u32; 3]) -> Result<(), String> {
    if index >= 8 {
        return Err("Hardware texture index exceeds eight slots".into());
    }
    stage[4] ^= (((index as u32) << 7) ^ stage[4]) & 0x1ffff80;
    if f32::from_bits(transform[0]) == 0. {
        stage[2] = (stage[2] & 0xfffe2fff) | 0x2000;
    } else {
        stage[2] = (stage[2] & 0xffff7fff) | 0x17000;
        stage[22] = transform[0];
        stage[23] = 0;
        stage[24] = 0;
        stage[25] = transform[0];
        stage[27] = transform[1];
        stage[26] = transform[2];
    }
    Ok(())
}
/// Stops at the first null material or resolver returning None. Prior writes persist.
/// Transform entries and existing stage words must be supplied; no class defaults inferred.
pub fn bind<T>(
    textures: &[Option<T>; 8],
    transforms: [[u32; 3]; 8],
    limit: usize,
    stages: &mut [Stage; 8],
    resolve: &mut Resolver<'_, T>,
) -> Result<usize, String> {
    if limit > 8 {
        return Err("Hardware stage capacity exceeds texture array".into());
    }
    let mut count = 0;
    for index in 0..limit {
        let Some(texture) = &textures[index] else {
            break;
        };
        let Some(mut stage) = resolve(index, texture)? else {
            break;
        };
        configure(&mut stage, index, transforms[index])?;
        stages[index] = stage;
        count += 1;
    }
    Ok(count)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn holes_and_resolver_failure_stop_without_touching_later_stages() {
        let textures = [Some(1), None, Some(3), None, None, None, None, None];
        let mut stages = [[99; 28]; 8];
        let mut calls = vec![];
        assert_eq!(
            bind(&textures, [[0; 3]; 8], 8, &mut stages, &mut |i, _| {
                calls.push(i);
                Ok(Some([0; 28]))
            })
            .unwrap(),
            1
        );
        assert_eq!(calls, [0]);
        assert_eq!(stages[1], [99; 28]);
        let textures = [Some(1); 8];
        calls.clear();
        assert_eq!(
            bind(&textures, [[0; 3]; 8], 8, &mut stages, &mut |i, _| {
                calls.push(i);
                Ok((i < 2).then_some([0; 28]))
            })
            .unwrap(),
            2
        );
        assert_eq!(calls, [0, 1, 2]);
        assert_eq!(stages[2], [99; 28]);
    }
    #[test]
    fn transform_word_order_signed_zero_and_nan_match_native_branch() {
        let mut s = [u32::MAX; 28];
        configure(&mut s, 3, [2f32.to_bits(), 11, 22]).unwrap();
        assert_eq!(s[22..28], [2f32.to_bits(), 0, 0, 2f32.to_bits(), 22, 11]);
        assert_eq!(s[4] & 0x1ffff80, 3 << 7);
        let mut z = s;
        configure(&mut z, 0, [0x80000000, 7, 8]).unwrap();
        assert_eq!(z[22..28], s[22..28]);
        assert_eq!(z[2], (s[2] & 0xfffe2fff) | 0x2000);
        configure(&mut z, 7, [0x7fc12345, 7, 8]).unwrap();
        assert_eq!(z[22], 0x7fc12345);
    }
    #[test]
    fn capacity_and_callback_error_preserve_completed_stages() {
        let mut s = [[99; 28]; 8];
        let textures = [Some(1); 8];
        assert_eq!(
            bind(&textures, [[0; 3]; 8], 0, &mut s, &mut |_, _| panic!()).unwrap(),
            0
        );
        assert!(bind(&textures, [[0; 3]; 8], 9, &mut s, &mut |_, _| panic!()).is_err());
        assert!(
            bind(&textures, [[0; 3]; 8], 8, &mut s, &mut |i, _| if i == 1 {
                Err("host".into())
            } else {
                Ok(Some([0; 28]))
            })
            .is_err()
        );
        assert_eq!(s[0][2], 0x2000);
        assert_eq!(s[1], [99; 28]);
    }
}
