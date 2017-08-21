// Copyright 2017 PingCAP, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// See the License for the specific language governing permissions and
// limitations under the License.

// FIXME(shirly): remove following later
#![allow(dead_code)]

use std::{str, i64, u64};
use std::ascii::AsciiExt;
use std::borrow::Cow;

use coprocessor::codec::{mysql, Datum};
use coprocessor::codec::mysql::{charset, types, Decimal, Duration, Json, Res, Time};
use coprocessor::codec::mysql::decimal::RoundMode;
use coprocessor::codec::convert::{self, convert_float_to_int, convert_float_to_uint,
                                  convert_int_to_uint};

use super::{FnCall, Result, StatementContext};

impl FnCall {
    pub fn cast_int_as_int(&self, ctx: &StatementContext, row: &[Datum]) -> Result<Option<i64>> {
        self.children[0].eval_int(ctx, row)
    }

    pub fn cast_real_as_int(&self, ctx: &StatementContext, row: &[Datum]) -> Result<Option<i64>> {
        let val = try!(self.children[0].eval_real(ctx, row));
        if val.is_none() {
            return Ok(None);
        }
        let val = val.unwrap();
        if mysql::has_unsigned_flag(self.tp.get_flag() as u64) {
            let uval = try!(convert_float_to_uint(val, u64::MAX, types::DOUBLE));
            Ok(Some(uval as i64))
        } else {
            let res = try!(convert_float_to_int(val, i64::MIN, i64::MAX, types::DOUBLE));
            Ok(Some(res))
        }
    }

    pub fn cast_decimal_as_int(
        &self,
        ctx: &StatementContext,
        row: &[Datum],
    ) -> Result<Option<i64>> {
        let val = try!(self.children[0].eval_decimal(ctx, row));
        if val.is_none() {
            return Ok(None);
        }
        let val = val.unwrap()
            .into_owned()
            .round(0, RoundMode::HalfEven)
            .unwrap();
        if mysql::has_unsigned_flag(self.tp.get_flag() as u64) {
            let uint = val.as_u64().unwrap();
            // TODO:handle overflow
            Ok(Some(uint as i64))
        } else {
            let val = val.as_i64().unwrap();
            // TODO:handle overflow
            Ok(Some(val))
        }
    }

    pub fn cast_str_as_int(&self, ctx: &StatementContext, row: &[Datum]) -> Result<Option<i64>> {
        if self.children[0].is_hybrid_type() {
            return self.children[0].eval_int(ctx, row);
        }
        let val = try!(self.children[0].eval_string(ctx, row));
        if val.is_none() {
            return Ok(None);
        }
        let val = val.unwrap();
        let negative_flag = b'-';
        let is_negative = match val.iter().skip_while(|x| x.is_ascii_whitespace()).next() {
            Some(&negative_flag) => true,
            _ => false,
        };
        if is_negative {
            // negative
            let v = try!(convert::bytes_to_int(ctx, &val));
            // TODO: if overflow, don't append this warning
            Ok(Some(v))
        } else {
            let urs = try!(convert::bytes_to_uint(ctx, &val));
            // TODO: process overflow
            Ok(Some(urs as i64))
        }
    }

    pub fn cast_time_as_int(&self, ctx: &StatementContext, row: &[Datum]) -> Result<Option<i64>> {
        let val = try!(self.children[0].eval_time(ctx, row));
        if val.is_none() {
            return Ok(None);
        }
        let dec = try!(val.unwrap().to_decimal());
        let dec = dec.round(mysql::DEFAULT_FSP as i8, RoundMode::HalfEven)
            .unwrap();
        let res = dec.as_i64().unwrap();
        Ok(Some(res))
    }

    pub fn cast_duration_as_int(
        &self,
        ctx: &StatementContext,
        row: &[Datum],
    ) -> Result<Option<i64>> {
        let val = try!(self.children[0].eval_duration(ctx, row));
        if val.is_none() {
            return Ok(None);
        }
        let dec = try!(val.unwrap().to_decimal());
        let dec = dec.round(mysql::DEFAULT_FSP as i8, RoundMode::HalfEven)
            .unwrap();
        let res = dec.as_i64().unwrap();
        Ok(Some(res))
    }

    pub fn cast_json_as_int(&self, ctx: &StatementContext, row: &[Datum]) -> Result<Option<i64>> {
        let val = try!(self.children[0].eval_json(ctx, row));
        if val.is_none() {
            return Ok(None);
        }
        let val = val.unwrap();
        let res = val.cast_to_int();
        Ok(Some(res))
    }

    pub fn cast_int_as_real(&self, ctx: &StatementContext, row: &[Datum]) -> Result<Option<f64>> {
        let val = try!(self.children[0].eval_int(ctx, row));
        if val.is_none() {
            return Ok(None);
        }
        let val = val.unwrap();
        if !mysql::has_unsigned_flag(self.children[0].get_tp().get_flag() as u64) {
            Ok(Some(
                try!(self.produce_float_with_specified_tp(ctx, val as f64)),
            ))
        } else {
            let uval = try!(convert_int_to_uint(val, u64::MAX, types::LONG_LONG));
            Ok(Some(
                try!(self.produce_float_with_specified_tp(ctx, uval as f64)),
            ))
        }
    }

    pub fn cast_real_as_real(&self, ctx: &StatementContext, row: &[Datum]) -> Result<Option<f64>> {
        let val = try!(self.children[0].eval_real(ctx, row));
        if val.is_none() {
            return Ok(None);
        }
        Ok(Some(try!(
            self.produce_float_with_specified_tp(ctx, val.unwrap())
        )))
    }

    pub fn cast_decimal_as_real(
        &self,
        ctx: &StatementContext,
        row: &[Datum],
    ) -> Result<Option<f64>> {
        let val = try!(self.children[0].eval_decimal(ctx, row));
        if val.is_none() {
            return Ok(None);
        }
        let val = val.unwrap();
        let res = try!(val.as_f64());
        Ok(Some(try!(self.produce_float_with_specified_tp(ctx, res))))
    }

    pub fn cast_str_as_real(&self, ctx: &StatementContext, row: &[Datum]) -> Result<Option<f64>> {
        if self.children[0].is_hybrid_type() {
            return self.children[0].eval_real(ctx, row);
        }
        let val = try!(self.children[0].eval_string(ctx, row));
        if val.is_none() {
            return Ok(None);
        }
        let val = val.unwrap();
        let res = try!(convert::bytes_to_f64(ctx, &val));
        Ok(Some(try!(self.produce_float_with_specified_tp(ctx, res))))
    }

    pub fn cast_time_as_real(&self, ctx: &StatementContext, row: &[Datum]) -> Result<Option<f64>> {
        let val = try!(self.children[0].eval_time(ctx, row));
        if val.is_none() {
            return Ok(None);
        }
        let val = try!(val.unwrap().to_decimal());
        let res = try!(val.as_f64());
        Ok(Some(try!(self.produce_float_with_specified_tp(ctx, res))))
    }

    pub fn cast_duration_as_real(
        &self,
        ctx: &StatementContext,
        row: &[Datum],
    ) -> Result<Option<f64>> {
        let val = try!(self.children[0].eval_duration(ctx, row));
        if val.is_none() {
            return Ok(None);
        }
        let val = try!(val.unwrap().to_decimal());
        let res = try!(val.as_f64());
        Ok(Some(try!(self.produce_float_with_specified_tp(ctx, res))))
    }

    pub fn cast_json_as_real(&self, ctx: &StatementContext, row: &[Datum]) -> Result<Option<f64>> {
        let val = try!(self.children[0].eval_json(ctx, row));
        if val.is_none() {
            return Ok(None);
        }
        let val = val.unwrap().cast_to_real();
        Ok(Some(try!(self.produce_float_with_specified_tp(ctx, val))))
    }

    pub fn cast_int_as_decimal<'a, 'b: 'a>(
        &'b self,
        ctx: &StatementContext,
        row: &'a [Datum],
    ) -> Result<Option<Cow<'a, Decimal>>> {
        let val = try!(self.children[0].eval_int(ctx, row));
        if val.is_none() {
            return Ok(None);
        }
        let val = val.unwrap();
        let field_type = &self.children[0].get_tp();
        let res = if !mysql::has_unsigned_flag(field_type.get_flag() as u64) {
            Decimal::from(val)
        } else {
            let uval = try!(convert_int_to_uint(val, u64::MAX, types::LONG_LONG));
            Decimal::from(uval)
        };
        Ok(Some(try!(self.produce_dec_with_specified_tp(ctx, res))))
    }

    pub fn cast_real_as_decimal<'a, 'b: 'a>(
        &'b self,
        ctx: &StatementContext,
        row: &'a [Datum],
    ) -> Result<Option<Cow<'a, Decimal>>> {
        let val = try!(self.children[0].eval_real(ctx, row));
        if val.is_none() {
            return Ok(None);
        }
        let res = try!(Decimal::from_f64(val.unwrap()));
        Ok(Some(try!(self.produce_dec_with_specified_tp(ctx, res))))
    }

    pub fn cast_decimal_as_decimal<'a, 'b: 'a>(
        &'b self,
        ctx: &StatementContext,
        row: &'a [Datum],
    ) -> Result<Option<Cow<'a, Decimal>>> {
        let val = try!(self.children[0].eval_decimal(ctx, row));
        if val.is_none() {
            return Ok(None);
        }
        Ok(Some(try!(self.produce_dec_with_specified_tp(
            ctx,
            val.unwrap().into_owned()
        ))))
    }

    pub fn cast_str_as_decimal<'a, 'b: 'a>(
        &'b self,
        ctx: &StatementContext,
        row: &'a [Datum],
    ) -> Result<Option<Cow<'a, Decimal>>> {
        if self.children[0].is_hybrid_type() {
            return self.children[0].eval_decimal(ctx, row);
        }
        let val = try!(self.children[0].eval_string(ctx, row));
        if val.is_none() {
            return Ok(None);
        }
        let bs = val.unwrap();
        let dec = match try!(Decimal::from_bytes(&bs)) {
            Res::Ok(d) | Res::Overflow(d) => d,
            Res::Truncated(d) => {
                try!(convert::handle_truncate(ctx, true));
                d
            }
        };
        Ok(Some(try!(self.produce_dec_with_specified_tp(ctx, dec))))
    }

    pub fn cast_time_as_decimal<'a, 'b: 'a>(
        &'b self,
        ctx: &StatementContext,
        row: &'a [Datum],
    ) -> Result<Option<Cow<'a, Decimal>>> {
        let val = try!(self.children[0].eval_time(ctx, row));
        if val.is_none() {
            return Ok(None);
        }
        let dec = try!(val.unwrap().to_decimal());
        Ok(Some(try!(self.produce_dec_with_specified_tp(ctx, dec))))
    }

    pub fn cast_duration_as_decimal<'a, 'b: 'a>(
        &'b self,
        ctx: &StatementContext,
        row: &'a [Datum],
    ) -> Result<Option<Cow<'a, Decimal>>> {
        let val = try!(self.children[0].eval_duration(ctx, row));
        if val.is_none() {
            return Ok(None);
        }
        let dec = try!(val.unwrap().to_decimal());
        Ok(Some(try!(self.produce_dec_with_specified_tp(ctx, dec))))
    }


    pub fn cast_json_as_decimal<'a, 'b: 'a>(
        &'b self,
        ctx: &StatementContext,
        row: &'a [Datum],
    ) -> Result<Option<Cow<'a, Decimal>>> {
        let val = try!(self.children[0].eval_json(ctx, row));
        if val.is_none() {
            return Ok(None);
        }
        let val = val.unwrap().cast_to_real();
        let dec = try!(Decimal::from_f64(val));
        Ok(Some(try!(self.produce_dec_with_specified_tp(ctx, dec))))
    }

    pub fn cast_int_as_str<'a, 'b: 'a>(
        &'b self,
        ctx: &StatementContext,
        row: &'a [Datum],
    ) -> Result<Option<Cow<'a, Vec<u8>>>> {
        let val = try!(self.children[0].eval_int(ctx, row));
        if val.is_none() {
            return Ok(None);
        }
        let s = if mysql::has_unsigned_flag(self.children[0].get_tp().get_flag() as u64) {
            let uval = try!(convert_int_to_uint(
                val.unwrap(),
                u64::MAX,
                types::LONG_LONG
            ));
            format!("{}", uval)
        } else {
            format!("{}", val.unwrap())
        };
        Ok(Some(try!(self.produce_str_with_specified_tp(ctx, s))))
    }

    pub fn cast_real_as_str<'a, 'b: 'a>(
        &'b self,
        ctx: &StatementContext,
        row: &'a [Datum],
    ) -> Result<Option<Cow<'a, Vec<u8>>>> {
        let val = try!(self.children[0].eval_real(ctx, row));
        if val.is_none() {
            return Ok(None);
        }
        let s = format!("{}", val.unwrap());
        Ok(Some(try!(self.produce_str_with_specified_tp(ctx, s))))
    }

    pub fn cast_decimal_as_str<'a, 'b: 'a>(
        &'b self,
        ctx: &StatementContext,
        row: &'a [Datum],
    ) -> Result<Option<Cow<'a, Vec<u8>>>> {
        let val = try!(self.children[0].eval_decimal(ctx, row));
        if val.is_none() {
            return Ok(None);
        }
        let s = val.unwrap().to_string();
        Ok(Some(try!(self.produce_str_with_specified_tp(ctx, s))))
    }

    pub fn cast_str_as_str<'a, 'b: 'a>(
        &'b self,
        ctx: &StatementContext,
        row: &'a [Datum],
    ) -> Result<Option<Cow<'a, Vec<u8>>>> {
        let val = try!(self.children[0].eval_string(ctx, row));
        if val.is_none() {
            return Ok(None);
        }
        let s = try!(String::from_utf8(val.unwrap().into_owned()));
        Ok(Some(try!(self.produce_str_with_specified_tp(ctx, s))))
    }

    pub fn cast_time_as_str<'a, 'b: 'a>(
        &'b self,
        ctx: &StatementContext,
        row: &'a [Datum],
    ) -> Result<Option<Cow<'a, Vec<u8>>>> {
        let val = try!(self.children[0].eval_time(ctx, row));
        if val.is_none() {
            return Ok(None);
        }
        let s = format!("{}", val.unwrap());
        Ok(Some(try!(self.produce_str_with_specified_tp(ctx, s))))
    }


    pub fn cast_duration_as_str<'a, 'b: 'a>(
        &'b self,
        ctx: &StatementContext,
        row: &'a [Datum],
    ) -> Result<Option<Cow<'a, Vec<u8>>>> {
        let val = try!(self.children[0].eval_duration(ctx, row));
        if val.is_none() {
            return Ok(None);
        }
        let s = format!("{}", val.unwrap());
        Ok(Some(try!(self.produce_str_with_specified_tp(ctx, s))))
    }

    pub fn cast_json_as_str<'a, 'b: 'a>(
        &'b self,
        ctx: &StatementContext,
        row: &'a [Datum],
    ) -> Result<Option<Cow<'a, Vec<u8>>>> {
        let val = try!(self.children[0].eval_json(ctx, row));
        if val.is_none() {
            return Ok(None);
        }
        let s = val.unwrap().to_string();
        Ok(Some(try!(self.produce_str_with_specified_tp(ctx, s))))
    }

    pub fn cast_int_as_time<'a, 'b: 'a>(
        &'b self,
        ctx: &StatementContext,
        row: &'a [Datum],
    ) -> Result<Option<Cow<'a, Time>>> {
        let val = try!(self.children[0].eval_int(ctx, row));
        if val.is_none() {
            return Ok(None);
        }
        let s = format!("{}", val.unwrap());
        Ok(Some(try!(self.produce_time_with_str(ctx, s))))
    }

    pub fn cast_real_as_time<'a, 'b: 'a>(
        &'b self,
        ctx: &StatementContext,
        row: &'a [Datum],
    ) -> Result<Option<Cow<'a, Time>>> {
        let val = try!(self.children[0].eval_real(ctx, row));
        if val.is_none() {
            return Ok(None);
        }
        let s = format!("{}", val.unwrap());
        Ok(Some(try!(self.produce_time_with_str(ctx, s))))
    }

    pub fn cast_decimal_as_time<'a, 'b: 'a>(
        &'b self,
        ctx: &StatementContext,
        row: &'a [Datum],
    ) -> Result<Option<Cow<'a, Time>>> {
        let val = try!(self.children[0].eval_decimal(ctx, row));
        if val.is_none() {
            return Ok(None);
        }
        let s = val.unwrap().to_string();
        Ok(Some(try!(self.produce_time_with_str(ctx, s))))
    }

    pub fn cast_str_as_time<'a, 'b: 'a>(
        &'b self,
        ctx: &StatementContext,
        row: &'a [Datum],
    ) -> Result<Option<Cow<'a, Time>>> {
        let val = try!(self.children[0].eval_string(ctx, row));
        if val.is_none() {
            return Ok(None);
        }
        let s = try!(String::from_utf8(val.unwrap().into_owned()));
        Ok(Some(try!(self.produce_time_with_str(ctx, s))))
    }

    pub fn cast_time_as_time<'a, 'b: 'a>(
        &'b self,
        ctx: &StatementContext,
        row: &'a [Datum],
    ) -> Result<Option<Cow<'a, Time>>> {
        let val = try!(self.children[0].eval_time(ctx, row));
        if val.is_none() {
            return Ok(None);
        }
        let mut val = val.unwrap().into_owned();
        try!(val.round_frac(self.tp.get_decimal() as i8));
        // TODO: tidb only update tp when tp is Date
        try!(val.set_tp(self.tp.get_tp() as u8));
        Ok(Some(Cow::Owned(val)))
    }

    pub fn cast_duration_as_time<'a, 'b: 'a>(
        &'b self,
        ctx: &StatementContext,
        row: &'a [Datum],
    ) -> Result<Option<Cow<'a, Time>>> {
        let val = try!(self.children[0].eval_duration(ctx, row));
        if val.is_none() {
            return Ok(None);
        }
        let mut val = try!(Time::from_duration(&ctx.tz, val.unwrap().as_ref()));
        try!(val.round_frac(self.tp.get_decimal() as i8));
        try!(val.set_tp(self.tp.get_tp() as u8));
        Ok(Some(Cow::Owned(val)))
    }

    pub fn cast_json_as_time<'a, 'b: 'a>(
        &'b self,
        ctx: &StatementContext,
        row: &'a [Datum],
    ) -> Result<Option<Cow<'a, Time>>> {
        let val = try!(self.children[0].eval_json(ctx, row));
        if val.is_none() {
            return Ok(None);
        }
        let s = try!(val.unwrap().unquote());
        Ok(Some(try!(self.produce_time_with_str(ctx, s))))
    }

    pub fn cast_int_as_duration<'a, 'b: 'a>(
        &'b self,
        ctx: &StatementContext,
        row: &'a [Datum],
    ) -> Result<Option<Cow<'a, Duration>>> {
        let val = try!(self.children[0].eval_int(ctx, row));
        if val.is_none() {
            return Ok(None);
        }
        let s = format!("{}", val.unwrap());
        let dur = try!(Duration::parse(s.as_bytes(), self.tp.get_decimal() as i8));
        Ok(Some(Cow::Owned(dur)))
    }

    pub fn cast_real_as_duration<'a, 'b: 'a>(
        &'b self,
        ctx: &StatementContext,
        row: &'a [Datum],
    ) -> Result<Option<Cow<'a, Duration>>> {
        let val = try!(self.children[0].eval_real(ctx, row));
        if val.is_none() {
            return Ok(None);
        }
        let s = format!("{}", val.unwrap());
        let dur = try!(Duration::parse(s.as_bytes(), self.tp.get_decimal() as i8));
        Ok(Some(Cow::Owned(dur)))
    }

    pub fn cast_decimal_as_duration<'a, 'b: 'a>(
        &'b self,
        ctx: &StatementContext,
        row: &'a [Datum],
    ) -> Result<Option<Cow<'a, Duration>>> {
        let val = try!(self.children[0].eval_decimal(ctx, row));
        if val.is_none() {
            return Ok(None);
        }
        let s = val.unwrap().to_string();
        let dur = try!(Duration::parse(s.as_bytes(), self.tp.get_decimal() as i8));
        Ok(Some(Cow::Owned(dur)))
    }

    pub fn cast_str_as_duration<'a, 'b: 'a>(
        &'b self,
        ctx: &StatementContext,
        row: &'a [Datum],
    ) -> Result<Option<Cow<'a, Duration>>> {
        let val = try!(self.children[0].eval_string(ctx, row));
        if val.is_none() {
            return Ok(None);
        }
        let val = val.unwrap();
        // TODO: tidb would handle truncate here
        let dur = try!(Duration::parse(val.as_ref(), self.tp.get_decimal() as i8));
        Ok(Some(Cow::Owned(dur)))
    }

    pub fn cast_time_as_duration<'a, 'b: 'a>(
        &'b self,
        ctx: &StatementContext,
        row: &'a [Datum],
    ) -> Result<Option<Cow<'a, Duration>>> {
        let val = try!(self.children[0].eval_time(ctx, row));
        if val.is_none() {
            return Ok(None);
        }
        let mut res = try!(val.unwrap().to_duration());
        try!(res.round_frac(self.tp.get_decimal() as i8));
        Ok(Some(Cow::Owned(res)))
    }

    pub fn cast_duration_as_duration<'a, 'b: 'a>(
        &'b self,
        ctx: &StatementContext,
        row: &'a [Datum],
    ) -> Result<Option<Cow<'a, Duration>>> {
        let val = try!(self.children[0].eval_duration(ctx, row));
        if val.is_none() {
            return Ok(None);
        }
        let mut res = val.unwrap().into_owned();
        try!(res.round_frac(self.tp.get_decimal() as i8));
        Ok(Some(Cow::Owned(res)))
    }

    pub fn cast_json_as_duration<'a, 'b: 'a>(
        &'b self,
        ctx: &StatementContext,
        row: &'a [Datum],
    ) -> Result<Option<Cow<'a, Duration>>> {
        let val = try!(self.children[0].eval_json(ctx, row));
        if val.is_none() {
            return Ok(None);
        }
        let s = try!(val.unwrap().unquote());
        // TODO: tidb would handle truncate here
        let d = try!(Duration::parse(s.as_bytes(), self.tp.get_decimal() as i8));
        Ok(Some(Cow::Owned(d)))
    }

    pub fn cast_int_as_jsonn<'a, 'b: 'a>(
        &'b self,
        ctx: &StatementContext,
        row: &'a [Datum],
    ) -> Result<Option<Cow<'a, Json>>> {
        let val = try!(self.children[0].eval_int(ctx, row));
        if val.is_none() {
            return Ok(None);
        }
        let j = if mysql::has_unsigned_flag(self.children[0].get_tp().get_flag() as u64) {
            Json::U64(val.unwrap() as u64)
        } else {
            Json::I64(val.unwrap())
        };
        Ok(Some(Cow::Owned(j)))
    }

    pub fn cast_real_as_json<'a, 'b: 'a>(
        &'b self,
        ctx: &StatementContext,
        row: &'a [Datum],
    ) -> Result<Option<Cow<'a, Json>>> {
        let val = try!(self.children[0].eval_real(ctx, row));
        if val.is_none() {
            return Ok(None);
        }
        Ok(Some(Cow::Owned(Json::Double(val.unwrap()))))
    }

    pub fn cast_decimal_as_json<'a, 'b: 'a>(
        &'b self,
        ctx: &StatementContext,
        row: &'a [Datum],
    ) -> Result<Option<Cow<'a, Json>>> {
        let val = try!(self.children[0].eval_decimal(ctx, row));
        if val.is_none() {
            return Ok(None);
        }
        let val = try!(val.unwrap().as_f64());
        Ok(Some(Cow::Owned(Json::Double(val))))
    }

    pub fn cast_str_as_json<'a, 'b: 'a>(
        &'b self,
        ctx: &StatementContext,
        row: &'a [Datum],
    ) -> Result<Option<Cow<'a, Json>>> {
        let val = try!(self.children[0].eval_string(ctx, row));
        if val.is_none() {
            return Ok(None);
        }
        let s = try!(String::from_utf8(val.unwrap().into_owned()));
        if self.tp.get_decimal() == 0 {
            let j: Json = try!(s.parse());
            Ok(Some(Cow::Owned(j)))
        } else {
            Ok(Some(Cow::Owned(Json::String(s))))
        }
    }

    pub fn cast_time_as_json<'a, 'b: 'a>(
        &'b self,
        ctx: &StatementContext,
        row: &'a [Datum],
    ) -> Result<Option<Cow<'a, Json>>> {
        let val = try!(self.children[0].eval_time(ctx, row));
        if val.is_none() {
            return Ok(None);
        }
        let mut val = val.unwrap().into_owned();
        if val.get_tp() == types::DATETIME || val.get_tp() == types::TIMESTAMP {
            val.set_fsp(mysql::MAX_FSP as u8);
        }
        let s = format!("{}", val);
        Ok(Some(Cow::Owned(Json::String(s))))
    }

    pub fn cast_duration_as_json<'a, 'b: 'a>(
        &'b self,
        ctx: &StatementContext,
        row: &'a [Datum],
    ) -> Result<Option<Cow<'a, Json>>> {
        let val = try!(self.children[0].eval_duration(ctx, row));
        if val.is_none() {
            return Ok(None);
        }
        let mut val = val.unwrap().into_owned();
        val.fsp = mysql::MAX_FSP as u8;
        let s = format!("{}", val);
        Ok(Some(Cow::Owned(Json::String(s))))
    }

    pub fn cast_json_as_json<'a, 'b: 'a>(
        &'b self,
        ctx: &StatementContext,
        row: &'a [Datum],
    ) -> Result<Option<Cow<'a, Json>>> {
        self.children[0].eval_json(ctx, row)
    }

    fn produce_dec_with_specified_tp(
        &self,
        ctx: &StatementContext,
        val: Decimal,
    ) -> Result<Cow<Decimal>> {
        let flen = self.tp.get_flen();
        let decimal = self.tp.get_decimal();
        if flen == convert::UNSPECIFIED_LENGTH || decimal == convert::UNSPECIFIED_LENGTH {
            return Ok(Cow::Owned(val));
        }
        let res = try!(val.convert_to(ctx, flen as u8, decimal as u8));
        Ok(Cow::Owned(res))
    }

    /// `produce_str_with_specified_tp`(`ProduceStrWithSpecifiedTp` in tidb) produces
    /// a new string according to `flen` and `chs`.
    fn produce_str_with_specified_tp(
        &self,
        ctx: &StatementContext,
        s: String,
    ) -> Result<Cow<Vec<u8>>> {
        let flen = self.tp.get_flen();
        let chs = self.tp.get_charset();
        if flen < 0 {
            return Ok(Cow::Owned(s.into_bytes()));
        }
        let flen = flen as usize;
        // flen is the char length, not byte length, for UTF8 charset, we need to calculate the
        // char count and truncate to flen chars if it is too long.
        if chs == charset::CHARSET_UTF8 || chs == charset::CHARSET_UTF8MB4 {
            let char_count = s.char_indices().count();
            if char_count <= flen {
                return Ok(Cow::Owned(s.into_bytes()));
            }

            if convert::handle_truncate_as_error(ctx) {
                return Err(box_err!(
                    "Data Too Long, field len {}, data len {}",
                    flen,
                    char_count
                ));
            }
            let (truncate_pos, _) = s.char_indices().nth(flen).unwrap();
            let res = convert::truncate_str(s, truncate_pos as isize).into_bytes();
            return Ok(Cow::Owned(res));
        }

        if s.len() > flen {
            if convert::handle_truncate_as_error(ctx) {
                return Err(box_err!(
                    "Data Too Long, field len {}, data len {}",
                    flen,
                    s.len()
                ));
            }
            let res = convert::truncate_str(s, flen as isize).into_bytes();
            return Ok(Cow::Owned(res));
        }

        if self.tp.get_tp() == types::STRING as i32 && s.len() < flen {
            let to_pad = flen - s.len();
            let mut ret = s.into_bytes();
            ret.append(&mut vec![0; to_pad]);
            return Ok(Cow::Owned(ret));
        }
        Ok(Cow::Owned(s.into_bytes()))
    }

    fn produce_time_with_str(&self, ctx: &StatementContext, s: String) -> Result<Cow<Time>> {
        // TODO: it's a bug in tidb do not care tz here
        let mut t = try!(Time::parse_datetime(
            s.as_ref(),
            self.tp.get_decimal() as i8,
            &ctx.tz
        ));
        try!(t.set_tp(self.tp.get_tp() as u8));
        Ok(Cow::Owned(t))
    }

    /// `produce_float_with_specified_tp`(`ProduceFloatWithSpecifiedTp` in tidb) produces
    /// a new float64 according to `flen` and `decimal` in `self.tp`.
    /// TODO port tests from tidb(tidb haven't implemented now)
    fn produce_float_with_specified_tp(&self, ctx: &StatementContext, f: f64) -> Result<f64> {
        let flen = self.tp.get_flen();
        let decimal = self.tp.get_decimal();
        if flen == convert::UNSPECIFIED_LENGTH || decimal == convert::UNSPECIFIED_LENGTH {
            return Ok(f);
        }
        match convert::truncate_f64(f, flen as u8, decimal as u8) {
            Res::Ok(d) => Ok(d),
            Res::Overflow(d) | Res::Truncated(d) => {
                //TODO process warning with ctx
                try!(convert::handle_truncate(ctx, true));
                Ok(d)
            }
        }
    }
}
